import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

// The panel is a native-feeling widget: no browser context menu.
document.addEventListener("contextmenu", (e) => {
  const t = e.target as HTMLElement | null;
  if (!t || !(t.tagName === "INPUT" || t.tagName === "TEXTAREA")) {
    e.preventDefault();
  }
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
