import { mount } from "svelte";
import App from "./components/App.svelte";
import "./styles/tokens.css";
import "./styles/base.css";
import { detectPlatform, detectPlatformSync } from "$lib/platform";

document.documentElement.dataset.platform = detectPlatformSync();
void detectPlatform();

const target = document.getElementById("app");
if (!target) throw new Error("#app no encontrado");

// Prototipo aislado de capa de texto: `npm run dev -- ?proto=text`
// (ver TECH_DEBT.md).
const isPrototype =
  import.meta.env.DEV &&
  new URLSearchParams(window.location.search).get("proto") === "text";

const app = isPrototype
  ? mount(
      (await import("$components/Prototype/TextLayerPrototype.svelte")).default,
      { target },
    )
  : mount(App, { target });

export default app;
