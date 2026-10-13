/// <reference path="../pb_data/types.d.ts" />

// Syncs Auth0-backed OpenID Connect provider slots onto the `users`
// collection from environment variables, on every start. This is
// deliberately a hook and not a migration: provider credentials are
// deployment configuration (they rotate), so they follow the environment
// instead of being baked in at the first migration run. When the variables
// are unset or empty, PocketBase is left untouched (password-only auth).
//
//   AUTH0_DOMAIN         Auth0 tenant domain, e.g. dev-abc123.us.auth0.com
//   AUTH0_CLIENT_ID      Auth0 application client id
//   AUTH0_CLIENT_SECRET  Auth0 application client secret
//
// Google/GitHub (and any other social logins) are configured as connections
// in the Auth0 dashboard; the `connection` query parameter baked into each
// slot's authURL routes the login straight to that connection, which is how
// one Auth0 application backs several labeled buttons.
//
// Redirect URIs to register in the Auth0 application (both of them):
//   <public_url>/auth/oauth/oidc/callback    (Google button)
//   <public_url>/auth/oauth/oidc2/callback   (GitHub button)

const AUTH0_SLOTS = [
    { name: "oidc", displayName: "Google", connection: "google-oauth2" },
    { name: "oidc2", displayName: "GitHub", connection: "github" },
]

// Runs when the web server starts: after bootstrap (db + migrations are
// ready) and before any request is served, so the first auth-methods lookup
// already sees the synced providers. Note: `app.onServe().bindFunc` keeps
// this closure alive on the hooks loader VM, unlike the `on*` global
// registration shorthand whose handlers are re-compiled per executor VM.
$app.onServe().bindFunc((e) => {
    const domain = ($os.getenv("AUTH0_DOMAIN") || "").trim()
    const clientId = ($os.getenv("AUTH0_CLIENT_ID") || "").trim()
    const clientSecret = ($os.getenv("AUTH0_CLIENT_SECRET") || "").trim()

    if (domain === "" || clientId === "" || clientSecret === "") {
        return e.next()
    }

    const base = (domain.startsWith("http") ? domain : "https://" + domain).replace(/\/+$/, "")

    try {
        const users = e.app.findCollectionByNameOrId("users")

        let providers = JSON.parse(JSON.stringify(users.oauth2.providers || []))
        let changed = false
        for (const slot of AUTH0_SLOTS) {
            const config = {
                name: slot.name,
                displayName: slot.displayName,
                clientId: clientId,
                clientSecret: clientSecret,
                authURL: base + "/authorize?connection=" + slot.connection,
                tokenURL: base + "/oauth/token",
                userInfoURL: base + "/userinfo",
                pkce: true,
            }
            const idx = providers.findIndex((p) => p.name === slot.name)
            if (idx >= 0) {
                if (JSON.stringify(providers[idx]) !== JSON.stringify(config)) {
                    providers[idx] = config
                    changed = true
                }
            } else {
                providers.push(config)
                changed = true
            }
        }

        if (users.oauth2.enabled !== true) {
            users.oauth2.enabled = true
            changed = true
        }

        if (changed) {
            users.oauth2.providers = providers
            e.app.save(users)
            console.log("auth0 hook: synced oidc oauth2 providers for", domain)
        }
    } catch (err) {
        console.error("auth0 hook: failed to sync oauth2 providers:", err)
    }

    return e.next()
})
