import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";

// Block the webview's default right-click menu (Reload, Inspect…) in release builds.
if (!import.meta.env.DEV) {
  document.addEventListener("contextmenu", (e) => {
    const t = e.target as HTMLElement;
    if (!t.closest("input, textarea")) e.preventDefault();
  });
}

export default mount(App, { target: document.getElementById("app")! });
