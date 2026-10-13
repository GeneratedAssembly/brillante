---
title: Whitelist Superuser Access by IP in Production
impact: MEDIUM
impactDescription: Restricts the most privileged account to known networks; limits blast radius of leaked superuser credentials
tags: production, security, superuser, ip-whitelist, cidr, v0.38
---

## Whitelist Superuser Access by IP in Production

PocketBase v0.38 added an optional superuser IPs/CIDR whitelist (Dashboard > Settings > Application > Superuser IPs). When set, superuser authentication and superuser-only APIs are only reachable from the listed addresses. If your team accesses the dashboard from stable IPs or a VPN, leaving this empty means a phished or leaked superuser password is usable from anywhere on the internet.

**Incorrect (public deployment, superuser API reachable from anywhere):**

```bash
# Settings > Application > Superuser IPs: (empty)
# Any IP that obtains superuser credentials can log in to /_/
# and use every superuser API.
```

**Correct (whitelist the VPN/CIDR, document the recovery path):**

```bash
# Settings > Application > Superuser IPs: 10.8.0.0/24, 203.0.113.7
# Superuser auth now only works from the VPN subnet + office IP.

# Locked out because your IP changed? Update the whitelist from the
# server shell (picked up live since v0.38, no restart needed):
./pocketbase superuser ips 10.8.0.0/24 198.51.100.20 --dir=/path/to/pb_data

# Clear the whitelist entirely:
./pocketbase superuser ips --dir=/path/to/pb_data
```

Pair this with a reverse proxy that forwards the real client IP (see deploy-reverse-proxy) — the whitelist checks the client IP PocketBase resolves, so a misconfigured proxy that reports its own IP will either lock everyone out or let everyone through.

Reference: [Going to production - Limit superusers to specific IPs/subnets](https://pocketbase.io/docs/going-to-production/#limit-superusers-to-specific-ipssubnets)
