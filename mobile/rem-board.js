{
  const BOOKMARK_NAME = "rem-cli-tasks";
  const STATUSES = Object.freeze(["parking", "todo", "doing", "done"]);
  const STATUS_LABELS = Object.freeze({
    parking: "PARKING",
    todo: "TODO",
    doing: "DOING",
    done: "DONE",
  });
  // Starts the Scriptable app, validates the file bookmark, renders the board, and listens for UI actions.
  const main = async () => {
    const fileManager = FileManager.iCloud();
    if (!fileManager.bookmarkExists(BOOKMARK_NAME)) {
      await showMissingBookmark(BOOKMARK_NAME);
      return;
    }
    const tasksRoot = fileManager.bookmarkedPath(BOOKMARK_NAME);
    await ensureStatusDirectories(fileManager, tasksRoot, STATUSES);
    const webView = new WebView();
    await webView.loadHTML(renderHtml(await loadState(fileManager, tasksRoot, STATUSES, STATUS_LABELS)));
    webView.present(true);
    await runActionLoop(webView, fileManager, tasksRoot, STATUSES, STATUS_LABELS);
  };
  // Shows setup guidance when the required Scriptable file bookmark is missing.
  const showMissingBookmark = async (bookmarkName) => {
    const alert = new Alert();
    alert.title = "File bookmark required";
    alert.message = `Create a Scriptable file bookmark named "${bookmarkName}" that points to the rem tasks directory.`;
    alert.addAction("OK");
    await alert.present();
  };
  // Creates the rem status directories when they do not already exist.
  const ensureStatusDirectories = async (fileManager, tasksRoot, statuses) => {
    statuses
      .map((status) => fileManager.joinPath(tasksRoot, status))
      .filter((path) => !fileManager.fileExists(path))
      .forEach((path) => fileManager.createDirectory(path, true));
  };
  // Loads every markdown task from each status directory into board state.
  const loadState = async (fileManager, tasksRoot, statuses, statusLabels) => {
    const entries = await Promise.all(statuses.map(async (status) => {
      const directory = fileManager.joinPath(tasksRoot, status);
      const fileNames = fileManager.listContents(directory);
      const tasks = await Promise.all(
        fileNames
          .filter((fileName) => fileName.endsWith(".md"))
          .map(async (fileName) => loadTask(fileManager, directory, fileName, status))
      );
      const sortedTasks = tasks
        .slice()
        .sort((left, right) => left.createdAt.localeCompare(right.createdAt));
      return [status, sortedTasks];
    }));
    return { tasksByStatus: Object.fromEntries(entries), statusLabels, statuses };
  };
  // Reads one task file and converts its frontmatter into a UI task object.
  const loadTask = async (fileManager, directory, fileName, status) => {
    const path = fileManager.joinPath(directory, fileName);
    await fileManager.downloadFileFromiCloud(path);
    const content = fileManager.readString(path);
    const parsed = parseTaskContent(content);
    return {
      id: parsed.frontmatter.id,
      name: parsed.frontmatter.name,
      status,
      createdAt: parsed.frontmatter.created_at,
      updatedAt: parsed.frontmatter.updated_at,
      completedAt: parsed.frontmatter.completed_at || null,
      deadline: parsed.frontmatter.deadline || "",
      path,
      fileName,
    };
  };
  // Splits a rem markdown file into frontmatter fields and markdown body.
  const parseTaskContent = (content) => {
    const match = content.match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);
    const yaml = match ? match[1] : "";
    const body = match ? match[2] : "";
    const frontmatter = Object.fromEntries(
      yaml
        .split("\n")
        .map((line) => line.match(/^([^:]+):\s*(.*)$/))
        .filter((matchResult) => matchResult)
        .map((matchResult) => [matchResult[1].trim(), parseYamlScalar(matchResult[2].trim())])
    );
    return { frontmatter, body };
  };
  // Parses the simple YAML scalar formats written by rem and this Scriptable script.
  const parseYamlScalar = (value) => {
    if (value === "") {
      return "";
    }
    if (value.startsWith("\"") && value.endsWith("\"")) {
      return JSON.parse(value);
    }
    if (value.startsWith("'") && value.endsWith("'")) {
      return value.slice(1, -1).replace(/''/g, "'");
    }
    return value;
  };
  // Waits for actions from the WebView, applies them to files, and refreshes the board state.
  const runActionLoop = async (webView, fileManager, tasksRoot, statuses, statusLabels) => {
    while (true) {
      const action = await waitForAction(webView);
      if (!action || action.type === "close") {
        return;
      }
      await handleAction(action, fileManager, tasksRoot, statuses);
      const state = await loadState(fileManager, tasksRoot, statuses, statusLabels);
      await webView.evaluateJavaScript(`window.remSetState(${JSON.stringify(state)})`, false);
    }
  };
  // Bridges the WebView action queue back to Scriptable through completion().
  const waitForAction = (webView) =>
    webView.evaluateJavaScript(
      `(() => {
        if (window.remPendingAction) {
          const action = window.remPendingAction;
          window.remPendingAction = null;
          completion(action);
          return;
        }
        window.remNativeCallback = completion;
      })();`,
      true
    );
  // Dispatches supported WebView actions through an extensible action registry.
  const actionHandlers = Object.freeze({
    add: (action, fileManager, tasksRoot) => addTask(fileManager, tasksRoot, action.name || ""),
    rename: (action, fileManager) => renameTask(fileManager, action.path, action.name || ""),
    move: (action, fileManager, tasksRoot, statuses) =>
      moveTask(fileManager, tasksRoot, statuses, action),
  });
  const handleAction = async (action, fileManager, tasksRoot, statuses) => {
    const handler = actionHandlers[action.type];
    if (handler) {
      await handler(action, fileManager, tasksRoot, statuses);
    }
  };
  // Creates a new TODO markdown file with rem-compatible frontmatter.
  const addTask = async (fileManager, tasksRoot, rawName) => {
    const name = rawName.trim();
    if (!name) {
      return;
    }
    const now = new Date();
    const id = UUID.string().toLowerCase();
    const task = {
      id,
      name,
      created_at: formatDateTime(now),
      updated_at: formatDateTime(now),
      deadline: formatDate(addDays(now, 1)),
    };
    const path = fileManager.joinPath(fileManager.joinPath(tasksRoot, "todo"), `${id}.md`);
    fileManager.writeString(path, buildTaskContent(task, ""));
  };
  // Updates the task title while preserving the existing markdown body.
  const renameTask = async (fileManager, path, rawName) => {
    const name = rawName.trim();
    if (!name) {
      return;
    }
    await fileManager.downloadFileFromiCloud(path);
    const parsed = parseTaskContent(fileManager.readString(path));
    const updated = {
      ...parsed.frontmatter,
      name,
      updated_at: formatDateTime(new Date()),
    };
    fileManager.writeString(path, buildTaskContent(updated, parsed.body));
  };
  // Returns the adjacent status for a valid lifecycle move.
  const transitionStatus = (statuses, status, direction) => {
    const currentIndex = statuses.indexOf(status);
    const nextIndex = currentIndex + direction;
    return currentIndex >= 0 && nextIndex >= 0 && nextIndex < statuses.length
      ? statuses[nextIndex]
      : null;
  };
  // Moves a task between status directories and updates completion metadata when needed.
  const moveTask = async (fileManager, tasksRoot, statuses, action) => {
    const newStatus = transitionStatus(statuses, action.status, action.direction);
    if (!newStatus) {
      return;
    }
    const destination = fileManager.joinPath(fileManager.joinPath(tasksRoot, newStatus), action.fileName);
    if (fileManager.fileExists(destination)) {
      throw new Error(`Destination already exists: ${destination}`);
    }
    await fileManager.downloadFileFromiCloud(action.path);
    const parsed = parseTaskContent(fileManager.readString(action.path));
    const now = new Date();
    const completedAt = action.status === "doing" && newStatus === "done"
      ? formatDateTime(now)
      : action.status === "done" && newStatus === "doing"
        ? null
        : parsed.frontmatter.completed_at;
    const updated = {
      ...parsed.frontmatter,
      updated_at: formatDateTime(now),
      completed_at: completedAt,
    };
    fileManager.writeString(action.path, buildTaskContent(updated, parsed.body));
    fileManager.move(action.path, destination);
  };
  // Builds a rem markdown document from frontmatter fields and the preserved body.
  const buildTaskContent = (frontmatter, body) => {
    const lines = [
      ["id", frontmatter.id],
      ["name", frontmatter.name],
      ["created_at", frontmatter.created_at],
      ["updated_at", frontmatter.updated_at],
      ["completed_at", frontmatter.completed_at],
      ["deadline", frontmatter.deadline],
    ]
      .filter((entry) => entry[1] !== undefined && entry[1] !== null && entry[1] !== "")
      .map(([key, value]) => `${key}: ${JSON.stringify(String(value))}`);
    return `---\n${lines.join("\n")}\n---\n${body || ""}`;
  };
  // Returns a new date shifted by the requested number of calendar days.
  const addDays = (date, days) => new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);
  // Formats a date as rem's yyyy/MM/dd deadline value.
  const formatDate = (date) => {
    const year = String(date.getFullYear()).padStart(4, "0");
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${year}/${month}/${day}`;
  };
  // Formats a local timestamp in the format rem can parse as NaiveDateTime.
  const formatDateTime = (date) => {
    const year = String(date.getFullYear()).padStart(4, "0");
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    const hours = String(date.getHours()).padStart(2, "0");
    const minutes = String(date.getMinutes()).padStart(2, "0");
    const seconds = String(date.getSeconds()).padStart(2, "0");
    return `${year}-${month}-${day}T${hours}:${minutes}:${seconds}`;
  };
  // Serializes state safely for an inline script element.
  const serializeScriptState = (state) =>
    JSON.stringify(state)
      .replace(/</g, "\\u003C")
      .replace(/\u2028/g, "\\u2028")
      .replace(/\u2029/g, "\\u2029");
  // Renders the complete HTML, CSS, and client-side JavaScript for the task board.
  const renderHtml = (state) => `<!doctype html>
<html>
<head>
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<style>
:root {
  color-scheme: light dark;
  --background: #f7f7f4;
  --foreground: #202124;
  --muted: #6b6f76;
  --line: #d9d7ce;
  --surface: #ffffff;
  --accent: #16697a;
  --parking: #8a5a44;
  --todo: #1d5f8a;
  --doing: #7d6b00;
  --done: #2e7d4f;
}
@media (prefers-color-scheme: dark) {
  :root {
    --background: #161716;
    --foreground: #f1f0ea;
    --muted: #a5a7aa;
    --line: #343630;
    --surface: #20221f;
    --accent: #5cc8d7;
  }
}
* {
  box-sizing: border-box;
}
body {
  margin: 0;
  background: var(--background);
  color: var(--foreground);
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
button,
input {
  font: inherit;
}
.app {
  min-height: 100vh;
  padding: max(12px, env(safe-area-inset-top)) 12px max(16px, env(safe-area-inset-bottom));
}
.toolbar {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
  margin-bottom: 12px;
}
.toolbar input {
  min-width: 0;
  border: 1px solid var(--line);
  border-radius: 6px;
  background: var(--surface);
  color: var(--foreground);
  padding: 10px 12px;
}
.toolbar button,
.task button {
  border: 1px solid var(--line);
  border-radius: 6px;
  background: var(--surface);
  color: var(--foreground);
  padding: 8px 10px;
}
.board {
  display: grid;
  grid-template-columns: repeat(4, minmax(210px, 1fr));
  gap: 10px;
  overflow-x: auto;
  padding-bottom: 12px;
}
.column {
  min-height: 72vh;
}
.column-header {
  position: sticky;
  top: 0;
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 2px;
  background: var(--background);
  font-size: 13px;
  font-weight: 700;
}
.column-header span:last-child {
  color: var(--muted);
  font-weight: 600;
}
.task {
  display: grid;
  gap: 8px;
  margin-bottom: 8px;
  border: 1px solid var(--line);
  border-left: 4px solid var(--accent);
  border-radius: 6px;
  background: var(--surface);
  padding: 10px;
}
.task[data-status="parking"] {
  border-left-color: var(--parking);
}
.task[data-status="todo"] {
  border-left-color: var(--todo);
}
.task[data-status="doing"] {
  border-left-color: var(--doing);
}
.task[data-status="done"] {
  border-left-color: var(--done);
}
.task-title {
  width: 100%;
  min-width: 0;
  border: 1px solid transparent;
  border-radius: 4px;
  background: transparent;
  color: var(--foreground);
  padding: 4px;
}
.task-title:focus {
  border-color: var(--accent);
  background: var(--background);
  outline: none;
}
.meta {
  color: var(--muted);
  font-size: 12px;
}
.actions {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 6px;
}
.actions button:disabled {
  opacity: 0.35;
}
</style>
</head>
<body>
<div class="app">
  <form id="new-task-form" class="toolbar">
    <input id="new-task" type="text" placeholder="New task" autocomplete="off">
    <button type="submit">Add</button>
  </form>
  <main id="board" class="board"></main>
</div>
<script>
const state = ${serializeScriptState(state)};
// Sends a browser-side action to Scriptable, or queues it until Scriptable is listening.
const sendAction = (action) => {
  if (window.remNativeCallback) {
    const callback = window.remNativeCallback;
    window.remNativeCallback = null;
    callback(action);
  } else {
    window.remPendingAction = action;
  }
};
// Handles the add form and sends a new task request to Scriptable.
document.getElementById("new-task-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const input = document.getElementById("new-task");
  sendAction({ type: "add", name: input.value });
  input.value = "";
});
// Creates one move button with its action bound without inline executable payloads.
const createMoveButton = (task, direction, disabled, label) => {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.disabled = disabled;
  button.addEventListener("click", () => sendAction({ type: "move", ...task, direction }));
  return button;
};
// Builds one task card using DOM properties for user-controlled values.
const createTaskElement = (task, status, index, statuses) => {
  const canMoveBack = index > 0;
  const canMoveForward = index < statuses.length - 1;
  const article = document.createElement("article");
  article.className = "task";
  article.dataset.status = status;
  const title = document.createElement("input");
  title.className = "task-title";
  title.value = task.name;
  title.addEventListener("change", () =>
    sendAction({ type: "rename", path: task.path, name: title.value })
  );
  const metadata = document.createElement("div");
  metadata.className = "meta";
  metadata.textContent = "Deadline: " + (task.deadline || "-");
  const actions = document.createElement("div");
  actions.className = "actions";
  actions.append(
    createMoveButton(task, -1, !canMoveBack, "←"),
    createMoveButton(task, 1, !canMoveForward, "→")
  );
  article.append(title, metadata, actions);
  return article;
};
// Builds one status column from trusted labels and DOM-safe task elements.
const createColumnElement = (nextState, status, index) => {
  const tasks = nextState.tasksByStatus[status] || [];
  const section = document.createElement("section");
  section.className = "column";
  const header = document.createElement("header");
  header.className = "column-header";
  const label = document.createElement("span");
  label.textContent = nextState.statusLabels[status];
  const count = document.createElement("span");
  count.textContent = String(tasks.length);
  header.append(label, count);
  section.append(
    header,
    ...tasks.map((task) => createTaskElement(task, status, index, nextState.statuses))
  );
  return section;
};
// Replaces the board contents with the latest Scriptable-provided state.
const render = (nextState) => {
  const board = document.getElementById("board");
  const columns = nextState.statuses.map((status, index) =>
    createColumnElement(nextState, status, index)
  );
  board.replaceChildren(...columns);
};
window.remSetState = (nextState) => render(nextState);
render(state);
</script>
</body>
</html>`;
  globalThis.remBoardTestApi = {
    addDays,
    buildTaskContent,
    formatDate,
    formatDateTime,
    parseTaskContent,
    parseYamlScalar,
    renderHtml,
    serializeScriptState,
    transitionStatus,
  };
  if (typeof process === "undefined" || !process.versions?.node) {
    await main();
  }
}
