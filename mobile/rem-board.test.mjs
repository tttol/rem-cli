import assert from "node:assert/strict";
import test from "node:test";

await import("./rem-board.js");

const {
  addDays,
  buildTaskContent,
  formatDate,
  formatDateTime,
  parseTaskContent,
  parseYamlScalar,
  renderHtml,
  serializeScriptState,
} = globalThis.remBoardTestApi;

test("parseYamlScalar parses rem-compatible scalar values", () => {
  // GIVEN
  const doubleQuoted = "\"task \\\"name\\\"\"";
  const singleQuoted = "'Bob''s task'";
  const plain = "2026/06/17";

  // WHEN
  const actual = [
    parseYamlScalar(doubleQuoted),
    parseYamlScalar(singleQuoted),
    parseYamlScalar(plain),
  ];

  // THEN
  const expected = ["task \"name\"", "Bob's task", "2026/06/17"];
  assert.deepEqual(actual, expected);
});

test("parseTaskContent splits frontmatter and markdown body", () => {
  // GIVEN
  const content = [
    "---",
    "id: \"task-id\"",
    "name: \"Review\"",
    "created_at: \"2026-06-16T10:00:00\"",
    "updated_at: \"2026-06-16T10:00:00\"",
    "deadline: \"2026/06/17\"",
    "---",
    "## Notes",
    "Keep this body.",
  ].join("\n");

  // WHEN
  const actual = parseTaskContent(content);

  // THEN
  const expected = {
    frontmatter: {
      id: "task-id",
      name: "Review",
      created_at: "2026-06-16T10:00:00",
      updated_at: "2026-06-16T10:00:00",
      deadline: "2026/06/17",
    },
    body: "## Notes\nKeep this body.",
  };
  assert.deepEqual(actual, expected);
});

test("buildTaskContent writes rem-compatible frontmatter and preserves body", () => {
  // GIVEN
  const frontmatter = {
    id: "task-id",
    name: "Review \"mobile\"",
    created_at: "2026-06-16T10:00:00",
    updated_at: "2026-06-16T11:00:00",
    completed_at: null,
    deadline: "2026/06/17",
  };
  const body = "## Notes\nKeep this body.";

  // WHEN
  const actual = buildTaskContent(frontmatter, body);

  // THEN
  const expected = [
    "---",
    "id: \"task-id\"",
    "name: \"Review \\\"mobile\\\"\"",
    "created_at: \"2026-06-16T10:00:00\"",
    "updated_at: \"2026-06-16T11:00:00\"",
    "deadline: \"2026/06/17\"",
    "---",
    "## Notes",
    "Keep this body.",
  ].join("\n");
  assert.equal(actual, expected);
});

test("date helpers format deadline and local timestamp values", () => {
  // GIVEN
  const date = new Date(2026, 5, 16, 7, 8, 9);
  const shifted = addDays(date, 1);

  // WHEN
  const actual = {
    deadline: formatDate(shifted),
    timestamp: formatDateTime(date),
  };

  // THEN
  const expected = {
    deadline: "2026/06/17",
    timestamp: "2026-06-16T07:08:09",
  };
  assert.deepEqual(actual, expected);
});

test("renderHtml includes client-side escaping and encoded payload handling", () => {
  // GIVEN
  const state = {
    statuses: ["todo"],
    statusLabels: { todo: "TODO" },
    tasksByStatus: {
      todo: [
        {
          id: "task-id",
          name: "Fix <quote> & \"title\"",
          status: "todo",
          createdAt: "2026-06-16T10:00:00",
          updatedAt: "2026-06-16T10:00:00",
          completedAt: null,
          deadline: "2026/06/17",
          path: "/tmp/todo/task-id.md",
          fileName: "task-id.md",
        },
      ],
    },
  };

  // WHEN
  const actual = renderHtml(state);

  // THEN
  assert.match(actual, /const payload = encodeURIComponent\(JSON\.stringify\(task\)\)/);
  assert.match(actual, /\.replace\(/);
  assert.match(actual, /&amp;/);
  assert.match(actual, /&lt;/);
  assert.match(actual, /&quot;/);
});

test("renderHtml safely serializes task names inside inline script state", () => {
  // GIVEN
  const state = {
    statuses: ["todo"],
    statusLabels: { todo: "TODO" },
    tasksByStatus: {
      todo: [
        {
          id: "task-id",
          name: "</script><script>window.injected = true</script>",
          status: "todo",
          createdAt: "2026-06-16T10:00:00",
          updatedAt: "2026-06-16T10:00:00",
          completedAt: null,
          deadline: "2026/06/17",
          path: "/tmp/todo/task-id.md",
          fileName: "task-id.md",
        },
      ],
    },
  };

  // WHEN
  const actual = renderHtml(state);

  // THEN
  assert.doesNotMatch(actual, /<\/script><script>window\.injected/);
  assert.match(actual, /\\u003C\/script>\\u003Cscript>window\.injected/);
});

test("serializeScriptState escapes characters that can break inline script parsing", () => {
  // GIVEN
  const state = {
    value: "<tag>\u2028line\u2029break",
  };

  // WHEN
  const actual = serializeScriptState(state);

  // THEN
  const expected = "{\"value\":\"\\u003Ctag>\\u2028line\\u2029break\"}";
  assert.equal(actual, expected);
});
