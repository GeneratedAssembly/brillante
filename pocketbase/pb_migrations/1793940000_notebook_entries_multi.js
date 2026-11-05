/// <reference path="../pb_data/types.d.ts" />

// Lessons hold multiple notes now. The unique (user, course, lesson) index
// enforced one replaceable note per lesson; drop it and keep a plain lookup
// index for owner-scoped list queries. Existing rows survive as the first
// note of their lesson.

migrate(
    (app) => {
        const collection = app.findCollectionByNameOrId("notebook_entries");
        collection.indexes = [
            "CREATE INDEX `idx_notebook_entries_user_course_lesson` ON `notebook_entries` (`user`, `course`, `lesson`)",
            "CREATE INDEX `idx_notebook_entries_user_course` ON `notebook_entries` (`user`, `course`)",
        ];
        app.save(collection);
    },
    (app) => {
        const collection = app.findCollectionByNameOrId("notebook_entries");
        collection.indexes = [
            "CREATE UNIQUE INDEX `idx_notebook_entries_user_course_lesson` ON `notebook_entries` (`user`, `course`, `lesson`)",
            "CREATE INDEX `idx_notebook_entries_user_course` ON `notebook_entries` (`user`, `course`)",
        ];
        app.save(collection);
    },
);
