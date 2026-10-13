/// <reference path="../pb_data/types.d.ts" />

// Let platform admins delete user accounts from the admin UI and see the
// fields managing requires. PocketBase hides auth fields (email) from
// viewers without manage access, so manageRule must match the admin rule or
// the users list renders blank emails. Every user-scoped collection
// (progress, course_grants, course_subscriptions, notebook_entries) has
// cascadeDelete on its user relation, so removing the auth record removes
// all of the user's rows. Brillante additionally refuses self-deletion and
// deleting the last remaining admin.

const ADMIN_RULE = '@request.auth.is_admin = true';

migrate(
    (app) => {
        const users = app.findCollectionByNameOrId("users");
        users.deleteRule = ADMIN_RULE;
        users.manageRule = ADMIN_RULE;
        app.save(users);
    },
    (app) => {
        const users = app.findCollectionByNameOrId("users");
        users.deleteRule = null;
        users.manageRule = null;
        app.save(users);
    },
);
