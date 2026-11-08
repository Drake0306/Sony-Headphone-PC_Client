import { mount } from "svelte";
import "./styles/tokens.css";
import "./styles/base.css";
import App from "./App.svelte";
import { installCrashOverlay, report } from "./lib/crash";

installCrashOverlay();

let app: ReturnType<typeof mount> | undefined;
try {
  app = mount(App, { target: document.getElementById("app")! });
} catch (e) {
  report(e);
}

export default app;
