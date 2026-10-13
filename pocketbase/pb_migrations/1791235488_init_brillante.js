/// <reference path="../pb_data/types.d.ts" />

// Brillante schema: per-user lesson completion.
//
// - `progress` holds one record per (user, course, lesson) that the user has
//   completed. All rules are ownership-scoped: a user can only ever see and
//   mutate their own rows (start locked, open selectively).
// - The `users` auth collection keeps its stock v0.40 defaults; open
//   self-registration (createRule "") is deliberate for a course platform.

migrate(
    (app) => {
        const progress = new Collection({
            type: "base",
            name: "progress",
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
                    pattern: "^[a-z0-9]+(?:-[a-z0-9]+)*$",
                },
                {
                    name: "lesson",
                    type: "text",
                    required: true,
                    system: false,
                    presentable: false,
                    min: null,
                    max: 160,
                    pattern: "^[a-z0-9]+(?:-[a-z0-9]+)*$",
                },
                {
                    name: "completed",
                    type: "autodate",
                    required: false,
                    system: false,
                    presentable: false,
                    onCreate: true,
                    onUpdate: false,
                },
            ],
            indexes: [
                "CREATE UNIQUE INDEX `idx_progress_user_course_lesson` ON `progress` (`user`, `course`, `lesson`)",
                "CREATE INDEX `idx_progress_user_course` ON `progress` (`user`, `course`)",
            ],
        });
        app.save(progress);
    },
    (app) => {
        const progress = app.findCollectionByNameOrId("progress");
        app.delete(progress);
    },
);
