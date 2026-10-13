/// <reference path="../pb_data/types.d.ts" />

// Phase 2 — per-user, per-course lesson notes. One entry per
// (user, course, lesson): the lesson page drawer creates or replaces it.
// Rules mirror course_subscriptions: everything is owner-scoped server-side;
// create pins the body's user to the token's id so a client can never name
// another user.

const SLUG_PATTERN = "^[a-z0-9]+(?:-[a-z0-9]+)*$";

migrate(
    (app) => {
        const collection = new Collection({
            type: "base",
            name: "notebook_entries",
            system: false,
            listRule: "user = @request.auth.id",
            viewRule: "user = @request.auth.id",
            createRule: '@request.auth.id != "" && @request.body.user = @request.auth.id',
            updateRule: "user = @request.auth.id",
            deleteRule: "user = @request.auth.id",
            fields: [
                {
                    name: "user",
                    type: "relation",
                    required: true,
                    system: false,
                    presentable: false,
                    collectionId: "_pb_users_auth_",
                    cascadeDelete: true,
                    minSelect: 1,
                    maxSelect: 1,
                },
                {
                    name: "course",
                    type: "text",
                    required: true,
                    system: false,
                    presentable: false,
                    hidden: false,
                    max: 160,
                    pattern: SLUG_PATTERN,
                },
                {
                    name: "lesson",
                    type: "text",
                    required: true,
                    system: false,
                    presentable: false,
                    hidden: false,
                    max: 160,
                    pattern: SLUG_PATTERN,
                },
                {
                    name: "anchor",
                    type: "text",
                    required: false,
                    system: false,
                    presentable: false,
                    hidden: false,
                    max: 160,
                    pattern: "",
                },
                {
                    name: "here_title",
                    type: "text",
                    required: false,
                    system: false,
                    presentable: false,
                    hidden: false,
                    max: 255,
                    pattern: "",
                },
                {
                    name: "body",
                    type: "text",
                    required: true,
                    system: false,
                    presentable: true,
                    hidden: false,
                    max: 65536,
                    pattern: "",
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
                {
                    name: "updated",
                    type: "autodate",
                    required: false,
                    system: false,
                    presentable: false,
                    onCreate: false,
                    onUpdate: true,
                },
            ],
            indexes: [
                "CREATE UNIQUE INDEX `idx_notebook_entries_user_course_lesson` ON `notebook_entries` (`user`, `course`, `lesson`)",
                "CREATE INDEX `idx_notebook_entries_user_course` ON `notebook_entries` (`user`, `course`)",
            ],
        });
        app.save(collection);
    },
    (app) => {
        const collection = app.findCollectionByNameOrId("notebook_entries");
        app.delete(collection);
    },
);
