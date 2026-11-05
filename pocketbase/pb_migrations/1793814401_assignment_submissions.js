/// <reference path="../pb_data/types.d.ts" />

// Assignment submissions: one row per (user, course, lesson, widget) holding
// the learner's latest answer and the grader's latest verdict. Rules mirror
// notebook_entries: everything is owner-scoped server-side; create pins the
// body's user to the token's id so a client can never name another user.

const SLUG_PATTERN = "^[a-z0-9]+(?:-[a-z0-9]+)*$";

migrate(
    (app) => {
        const collection = new Collection({
            type: "base",
            name: "assignment_submissions",
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
                    name: "widget",
                    type: "text",
                    required: true,
                    system: false,
                    presentable: false,
                    hidden: false,
                    max: 300,
                    pattern: "",
                },
                {
                    name: "answer",
                    type: "text",
                    required: false,
                    system: false,
                    presentable: false,
                    hidden: false,
                    max: 16384,
                    pattern: "",
                },
                {
                    name: "verdict",
                    type: "text",
                    required: false,
                    system: false,
                    presentable: false,
                    hidden: false,
                    max: 40,
                    pattern: "",
                },
                {
                    name: "score",
                    type: "number",
                    required: false,
                    system: false,
                    presentable: false,
                    hidden: false,
                    min: 0,
                    max: 1,
                },
                {
                    name: "feedback",
                    type: "text",
                    required: false,
                    system: false,
                    presentable: false,
                    hidden: false,
                    max: 16384,
                    pattern: "",
                },
                {
                    name: "attempt",
                    type: "number",
                    required: false,
                    system: false,
                    presentable: false,
                    hidden: false,
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
                "CREATE UNIQUE INDEX `idx_assignment_submissions_user_course_lesson_widget` ON `assignment_submissions` (`user`, `course`, `lesson`, `widget`)",
                "CREATE INDEX `idx_assignment_submissions_user_course` ON `assignment_submissions` (`user`, `course`)",
            ],
        });
        app.save(collection);
    },
    (app) => {
        const collection = app.findCollectionByNameOrId("assignment_submissions");
        app.delete(collection);
    },
);
