import "./lib/crash-install";
import { mount } from "svelte";
import "./styles/tokens.css";
import "./styles/base.css";
import App from "./App.svelte";
import { markMounted, report } from "./lib/crash";

let app: ReturnType<typeof mount> | undefined;
try {
  app = mount(App, { target: document.getElementById("app")! });
  markMounted();
} catch (e) {
  report(e);
}

export default app;
