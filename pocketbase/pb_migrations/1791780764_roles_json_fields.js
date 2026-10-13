/// <reference path="../pb_data/types.d.ts" />

// Fix the multi-value role fields created by
// 1791307800_roles_access_subscriptions.js.
//
// PocketBase 0.23+ removed multi-select from text fields: TextField has no
// maxSelect property, so `users.roles` and `course_access.allow_roles` were
// silently created as single-value text. Records then serialize them as plain
// strings ("") and every Brillante auth response fails to decode into
// Vec<String>.
//
// Free-form multi-value fields must be JSON arrays in this version, but PB
// 0.40.4 filter rules have no exact element-matching operator for JSON
// arrays (verified live: `?=` matches nothing, `~` matches substrings so
// ["superadmin"] would match "admin", and array_has() does not exist). So
// API rules use the documented fallback from the plan: a mirrored boolean
// `is_admin` maintained by Brillante on every role write, while the free-form
// `roles` vocabulary stays a JSON array matched in Rust (can_view).

const ADMIN_RULE = '@request.auth.is_admin = true';

migrate(
    (app) => {
        const users = app.findCollectionByNameOrId("users");
        users.fields.removeByName("roles");
        users.fields.add(
            new JSONField({
                name: "roles",
                maxSize: 2048,
                presentable: false,
            })
        );
        users.fields.add(
            new BoolField({
                name: "is_admin",
                presentable: false,
            })
        );
        users.listRule = `id = @request.auth.id || ${ADMIN_RULE}`;
        users.viewRule = `id = @request.auth.id || ${ADMIN_RULE}`;
        users.updateRule = ADMIN_RULE;
        users.createRule =
            "@request.body.roles:isset = false && @request.body.is_admin:isset = false";
        app.save(users);

        const courseAccess = app.findCollectionByNameOrId("course_access");
        courseAccess.fields.removeByName("allow_roles");
        courseAccess.fields.add(
            new JSONField({
                name: "allow_roles",
                maxSize: 4096,
                presentable: false,
            })
        );
        courseAccess.createRule = ADMIN_RULE;
        courseAccess.updateRule = ADMIN_RULE;
        courseAccess.deleteRule = ADMIN_RULE;
        app.save(courseAccess);

        const courseGrants = app.findCollectionByNameOrId("course_grants");
        courseGrants.listRule = `user = @request.auth.id || ${ADMIN_RULE}`;
        courseGrants.viewRule = `user = @request.auth.id || ${ADMIN_RULE}`;
        courseGrants.createRule = ADMIN_RULE;
        courseGrants.deleteRule = ADMIN_RULE;
        app.save(courseGrants);
    },
    (app) => {
        const users = app.findCollectionByNameOrId("users");
        users.fields.removeByName("roles");
        users.fields.removeByName("is_admin");
        users.fields.add(
            new TextField({
                name: "roles",
                max: 64,
                presentable: false,
            })
        );
        users.listRule = 'id = @request.auth.id || @request.auth.roles ?= "admin"';
        users.viewRule = 'id = @request.auth.id || @request.auth.roles ?= "admin"';
        users.updateRule = '@request.auth.roles ?= "admin"';
        users.createRule = "@request.body.roles:isset = false";
        app.save(users);

        const courseAccess = app.findCollectionByNameOrId("course_access");
        courseAccess.fields.removeByName("allow_roles");
        courseAccess.fields.add(
            new TextField({
                name: "allow_roles",
                max: 64,
                presentable: false,
            })
        );
        courseAccess.createRule = '@request.auth.roles ?= "admin"';
        courseAccess.updateRule = '@request.auth.roles ?= "admin"';
        courseAccess.deleteRule = '@request.auth.roles ?= "admin"';
        app.save(courseAccess);

        const courseGrants = app.findCollectionByNameOrId("course_grants");
        courseGrants.listRule = 'user = @request.auth.id || @request.auth.roles ?= "admin"';
        courseGrants.viewRule = 'user = @request.auth.id || @request.auth.roles ?= "admin"';
        courseGrants.createRule = '@request.auth.roles ?= "admin"';
        courseGrants.deleteRule = '@request.auth.roles ?= "admin"';
        app.save(courseGrants);
    },
);
