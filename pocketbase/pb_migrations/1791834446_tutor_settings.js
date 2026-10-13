/// <reference path="../pb_data/types.d.ts" />

// Virtual tutor configuration: one singleton row managed from Admin → Tutor.
//
// The api_key field is `hidden`: PocketBase 0.40.4 strips it from every API
// response AND silently drops writes from non-superuser tokens — so the
// server persists settings with its superuser token (same trust model as
// bootstrap_admins), while the admin gate is enforced by Brillante itself. Reads are public because enabled/base_url/model are not secrets and
// the lesson pages must know whether the tutor is available without an
// admin token; key_tail (last 4 chars) exists purely so the admin UI can
// show "••••abcd" vs "not set".

const ADMIN_RULE = '@request.auth.is_admin = true';

migrate(
    (app) => {
        const collection = new Collection({
            type: "base",
            name: "tutor_settings",
            system: false,
            listRule: "",
            viewRule: "",
            createRule: ADMIN_RULE,
            updateRule: ADMIN_RULE,
            deleteRule: null,
            fields: [
                {
                    name: "enabled",
                    type: "bool",
                    required: false,
                    system: false,
                    presentable: false,
                },
                {
                    name: "base_url",
                    type: "text",
                    required: false,
                    system: false,
                    presentable: false,
                    hidden: false,
                    max: 300,
                    pattern: "^https?://[^\\s]+$",
                },
                {
                    name: "api_key",
                    type: "text",
                    required: false,
                    system: false,
                    presentable: false,
                    hidden: true,
                    max: 400,
                    pattern: "",
                },
                {
                    name: "key_tail",
                    type: "text",
                    required: false,
                    system: false,
                    presentable: false,
                    hidden: false,
                    max: 8,
                    pattern: "",
                },
                {
                    name: "model",
                    type: "text",
                    required: false,
                    system: false,
                    presentable: false,
                    hidden: false,
                    max: 120,
                    pattern: "",
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
        });
        app.save(collection);
    },
    (app) => {
        const collection = app.findCollectionByNameOrId("tutor_settings");
        app.delete(collection);
    },
);
