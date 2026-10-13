/// <reference path="../pb_data/types.d.ts" />

// Roles, course privacy, grants, and subscriptions.
//
// - `users` gains a multi-value `roles` text field (special value `admin`;
//   other values are free-form course-access roles matched exactly against a
//   course's `allow_roles`). Rules are tightened so only admins may edit user
//   records (no self-role escalation) and signup bodies may not submit roles.
//   `?=` is the any-of operator for array-like fields; `:each` does not apply
//   to text fields, so rules use `@request.auth.roles ?= "admin"`.
// - `course_access` holds per-course privacy config keyed by the Brillante
//   course slug. List/view stay open so the server can render the catalog for
//   anonymous visitors (this exposes visibility + role vocabulary only, never
//   user emails). Writes are admin-gated.
// - `course_grants` opens a private course to an individual user. Users see
//   only their own grants; admins manage all of them. Grants are never
//   updated, only created and deleted.
// - `course_subscriptions` is the personal "my courses" list: strictly
//   user-owned, one row per (user, course), like `progress`.
//
// A course with no `course_access` row is public (backwards compatible).

const ADMIN_RULE = '@request.auth.roles ?= "admin"';
const SLUG_PATTERN = "^[a-z0-9]+(?:-[a-z0-9]+)*$";

migrate(
    (app) => {
        const users = app.findCollectionByNameOrId("users");
        users.listRule = 'id = @request.auth.id || @request.auth.roles ?= "admin"';
        users.viewRule = 'id = @request.auth.id || @request.auth.roles ?= "admin"';
        users.updateRule = ADMIN_RULE;
        users.createRule = "@request.body.roles:isset = false";
        users.fields.add(
            new TextField({
                name: "roles",
                maxSelect: 16,
                max: 64,
                presentable: false,
            })
        );
        app.save(users);

        const courseAccess = new Collection({
            type: "base",
            name: "course_access",
            system: false,
            listRule: "",
            viewRule: "",
            createRule: ADMIN_RULE,
            updateRule: ADMIN_RULE,
            deleteRule: ADMIN_RULE,
            fields: [
                {
                    name: "course",
                    type: "text",
                    required: true,
                    system: false,
                    presentable: false,
                    min: null,
                    max: 160,
                    pattern: SLUG_PATTERN,
                },
                {
                    name: "visibility",
                    type: "select",
                    required: true,
                    system: false,
                    presentable: false,
                    maxSelect: 1,
                    values: ["public", "private"],
                },
                {
                    name: "allow_roles",
                    type: "text",
                    required: false,
                    system: false,
                    presentable: false,
                    min: null,
                    max: 64,
                    maxSelect: 32,
                },
            ],
            indexes: [
                "CREATE UNIQUE INDEX `idx_course_access_course` ON `course_access` (`course`)",
            ],
        });
        app.save(courseAccess);

        const courseGrants = new Collection({
            type: "base",
            name: "course_grants",
            system: false,
            listRule: 'user = @request.auth.id || @request.auth.roles ?= "admin"',
            viewRule: 'user = @request.auth.id || @request.auth.roles ?= "admin"',
            createRule: ADMIN_RULE,
            updateRule: null,
            deleteRule: ADMIN_RULE,
            fields: [
                {
                    name: "user",
                    type: "relation",
                    required: true,
                    system: false,
                    presentable: false,
                    unique: false,
                    collectionId: "_pb_users_auth_",
                    cascadeDelete: true,
                    minSelect: null,
                    maxSelect: 1,
                    displayFields: null,
                },
                {
                    name: "course",
                    type: "text",
                    required: true,
                    system: false,
                    presentable: false,
                    min: null,
                    max: 160,
                    pattern: SLUG_PATTERN,
                },
            ],
            indexes: [
                "CREATE UNIQUE INDEX `idx_course_grants_user_course` ON `course_grants` (`user`, `course`)",
            ],
        });
        app.save(courseGrants);

        const subscriptions = new Collection({
            type: "base",
            name: "course_subscriptions",
            system: false,
            listRule: "user = @request.auth.id",
            viewRule: "user = @request.auth.id",
            createRule: '@request.auth.id != "" && @request.body.user = @request.auth.id',
            updateRule: "user = @request.auth.id && @request.body.user:changed = false",
            deleteRule: "user = @request.auth.id",
            fields: [
                {
                    name: "user",
                    type: "relation",
                    required: true,
                    system: false,
                    presentable: false,
                    unique: false,
                    collectionId: "_pb_users_auth_",
                    cascadeDelete: true,
                    minSelect: null,
                    maxSelect: 1,
                    displayFields: null,
                },
                {
                    name: "course",
                    type: "text",
                    required: true,
                    system: false,
                    presentable: false,
                    min: null,
                    max: 160,
                    pattern: SLUG_PATTERN,
                },
                {
                    name: "created",
                    type: "autodate",
                    required: false,
                    system: false,
                    presentable: false,
                    onCreate: true,
                    onUpdate: false,
                },
            ],
            indexes: [
                "CREATE UNIQUE INDEX `idx_course_subscriptions_user_course` ON `course_subscriptions` (`user`, `course`)",
            ],
        });
        app.save(subscriptions);
    },
    (app) => {
        for (const name of ["course_subscriptions", "course_grants", "course_access"]) {
            try {
                app.delete(app.findCollectionByNameOrId(name));
            } catch {
                // already gone
            }
        }
        const users = app.findCollectionByNameOrId("users");
        users.listRule = "id = @request.auth.id";
        users.viewRule = "id = @request.auth.id";
        users.updateRule = "id = @request.auth.id";
        users.createRule = "";
        try {
            users.fields.removeByName("roles");
        } catch {
            // already removed
        }
        app.save(users);
    },
);
