import React from "react";
import ReactDOM from "react-dom/client";
import { ThemeProvider } from "./contexts";
import Pill from "./Pill";
import Panel from "./Panel";
import "./global.css";
import { getCurrentWindow } from "@tauri-apps/api/window";

const currentWindow = getCurrentWindow();
const windowLabel = currentWindow.label;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      {windowLabel === "dashboard" ? <Panel /> : <Pill />}
    </ThemeProvider>
  </React.StrictMode>
);
