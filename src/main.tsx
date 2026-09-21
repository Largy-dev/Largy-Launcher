import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { applyAccent, loadAccent } from "./lib/theme";

const prefersDark = window.matchMedia("(prefers-color-scheme: dark)");
document.documentElement.classList.toggle("dark", prefersDark.matches);
prefersDark.addEventListener("change", (e) => {
  document.documentElement.classList.toggle("dark", e.matches);
});

applyAccent(loadAccent());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
