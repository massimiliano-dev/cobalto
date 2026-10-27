import "@fontsource-variable/instrument-sans/wght.css";
import "@fontsource/instrument-serif/latin-400.css";
import "@fontsource/instrument-serif/latin-400-italic.css";
import "./app.css";
import { mount } from "svelte";
import App from "./App.svelte";

// Fuori da Tauri, in sviluppo, si lavora sulla grafica con dati finti
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window && (window as any).__TAURI_INTERNALS__?.metadata)) {
  const { installPreview } = await import("./dev/preview");
  installPreview();
}

const app = mount(App, { target: document.getElementById("app")! });

export default app;
