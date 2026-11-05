/// <reference path="../pb_data/types.d.ts" />

// Dual-engine grader settings: the verdict comes from a Decisions model
// (typesafe/jev-1.13 via OpenRouter's /api/alpha/decisions, which returns
// only typed answers — no free text), while the worked feedback comes from
// an OpenAI-compatible chat model through {base_url}/chat/completions.
// chat_model is the chat engine (also the sole engine for chat-only setups);
// decisions_url + model are optional and select the Jev verdict engine.

migrate(
    (app) => {
        const collection = app.findCollectionByNameOrId("grader_settings");

        collection.fields.add(new Field({
            name: "chat_model",
            type: "text",
            required: false,
            system: false,
            presentable: false,
            hidden: false,
            max: 120,
            pattern: "",
        }));

        collection.fields.add(new Field({
            name: "decisions_url",
            type: "text",
            required: false,
            system: false,
            presentable: false,
            hidden: false,
            max: 300,
            pattern: "^https?://[^\\s]+$",
        }));

        app.save(collection);
    },
    (app) => {
        const collection = app.findCollectionByNameOrId("grader_settings");
        collection.fields.removeByName("chat_model");
        collection.fields.removeByName("decisions_url");
        app.save(collection);
    },
);
