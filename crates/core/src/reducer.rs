//! Single mutation path for projection (plan §4).

use uuid::Uuid;

use crate::error::ApplyError;
use crate::event::{CommandIoPhase, FactEvent};
use crate::state::State;
use crate::view::StateView;

fn command_io_key(command_id: &Option<Uuid>, room: &Option<String>) -> Option<String> {
    command_id
        .map(|id| format!("cmd:{id}"))
        .or_else(|| room.as_ref().map(|r| format!("room:{r}")))
}

fn validate_command_io(
    state: &State,
    command_id: &Option<Uuid>,
    room: &Option<String>,
    phase: &CommandIoPhase,
) -> Result<(), ApplyError> {
    let Some(k) = command_io_key(command_id, room) else {
        return Ok(());
    };
    let t = state
        .command_io_trackers
        .get(&k)
        .cloned()
        .unwrap_or_default();
    match phase {
        CommandIoPhase::Dispatched { .. } => {
            if t.awaiting_terminal {
                return Err(ApplyError::IoDuplicateDispatch(k));
            }
            if t.timeouts_seen >= 2 {
                return Err(ApplyError::IoRetryBudgetExhausted(k));
            }
            Ok(())
        }
        CommandIoPhase::Acked | CommandIoPhase::Failed { .. } | CommandIoPhase::Timeout => {
            if !t.awaiting_terminal {
                return Err(ApplyError::IoTerminalWithoutOpenDispatch(k));
            }
            Ok(())
        }
    }
}

fn apply_command_io_trackers(
    next: &mut State,
    command_id: &Option<Uuid>,
    room: &Option<String>,
    phase: &CommandIoPhase,
) {
    let Some(k) = command_io_key(command_id, room) else {
        return;
    };
    let mut t = next
        .command_io_trackers
        .get(&k)
        .cloned()
        .unwrap_or_default();
    match phase {
        CommandIoPhase::Dispatched { .. } => {
            t.awaiting_terminal = true;
            next.command_io_trackers.insert(k, t);
        }
        CommandIoPhase::Acked | CommandIoPhase::Failed { .. } => {
            next.command_io_trackers.remove(&k);
        }
        CommandIoPhase::Timeout => {
            t.awaiting_terminal = false;
            t.timeouts_seen = t.timeouts_seen.saturating_add(1);
            next.command_io_trackers.insert(k, t);
        }
    }
}

/// Validate fact before journal append (plan §4.3).
pub fn validate_fact_for_append(state: &State, fact: &FactEvent) -> Result<(), ApplyError> {
    match fact {
        FactEvent::LightOn { room, .. } => {
            if state.light_on(room) {
                return Err(ApplyError::LightAlreadyOn(room.clone()));
            }
            Ok(())
        }
        FactEvent::LightOff { room, .. } => {
            if !state.light_on(room) {
                return Err(ApplyError::LightAlreadyOff(room.clone()));
            }
            Ok(())
        }
        FactEvent::CommandIo {
            command_id,
            room,
            phase,
            ..
        } => validate_command_io(state, command_id, room, phase),
        FactEvent::UsageLogged { .. } | FactEvent::StateCorrectedFromObservation { .. } => Ok(()),
    }
}

/// Apply fact to state — functional step (plan §4.1).
pub fn apply_event(state: &State, fact: &FactEvent) -> Result<State, ApplyError> {
    let mut next = state.clone();
    match fact {
        FactEvent::LightOn { room, provenance } => {
            if next.lights.get(room).copied().unwrap_or(false) {
                return Err(ApplyError::LightAlreadyOn(room.clone()));
            }
            next.lights.insert(room.clone(), true);
            next.light_last_provenance.insert(room.clone(), *provenance);
        }
        FactEvent::LightOff { room, provenance } => {
            if !next.lights.get(room).copied().unwrap_or(false) {
                return Err(ApplyError::LightAlreadyOff(room.clone()));
            }
            next.lights.insert(room.clone(), false);
            next.light_last_provenance.insert(room.clone(), *provenance);
        }
        FactEvent::UsageLogged { item, .. } => {
            next.last_log = Some(item.clone());
        }
        FactEvent::CommandIo {
            command_id,
            room,
            phase,
            ..
        } => {
            validate_command_io(&next, command_id, room, phase)?;
            apply_command_io_trackers(&mut next, command_id, room, phase);
        }
        FactEvent::StateCorrectedFromObservation { .. } => {}
    }
    Ok(next)
}
