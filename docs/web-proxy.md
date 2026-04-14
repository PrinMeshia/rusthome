# Securing the lab web UI (reverse proxy)

`rusthome serve` and `rusthome-web` have **no built-in authentication**. Default bind **`127.0.0.1:8080`** is for local use only.

If you need the dashboard on a **LAN or the internet**, do **not** rely on `--bind 0.0.0.0` alone. Run rusthome on loopback and terminate **TLS** and **access control** in a reverse proxy.

## Pattern

1. Start rusthome on loopback only:

   ```bash
   rusthome serve --data-dir /path/to/data --bind 127.0.0.1:8080
   ```

2. Configure Caddy or nginx to listen on `:443` (or a LAN IP + TLS) and `reverse_proxy` / `proxy_pass` to `http://127.0.0.1:8080`.

3. Add **Basic Auth**, **OAuth2 proxy**, **mTLS**, or **IP allowlist** at the proxy (not in rusthome).

## Example files (copy and edit)

| File | Purpose |
|------|---------|
| [../configs/Caddyfile.rusthome.example](../configs/Caddyfile.rusthome.example) | Caddy 2: HTTPS (Let’s Encrypt or internal CA), optional Basic Auth |
| [../configs/nginx-rusthome.conf.example](../configs/nginx-rusthome.conf.example) | nginx: TLS + `proxy_pass` + optional `auth_basic` |

Replace hostnames, certificate paths, and credentials before use. For home LAN-only, a **private CA** or **Caddy internal** TLS is enough; for WAN, prefer Let’s Encrypt and strong auth.

## Firewall

Allow **only** the proxy ports (e.g. 443) from the networks you trust; keep **8080** closed from outside the host.

See also [integration.md — Web dashboard](integration.md#web-dashboard-lab) and [implementation.md — rusthome-web](implementation.md#rusthome-web-read-only-ui).
