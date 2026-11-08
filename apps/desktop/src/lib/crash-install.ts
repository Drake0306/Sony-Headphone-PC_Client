// Imported first by main.ts: ES imports run before a module's body, so the
// error catcher has to live in a module of its own to be in place before
// App.svelte and everything it imports are evaluated.
import { installCrashOverlay } from "./crash";

installCrashOverlay();
