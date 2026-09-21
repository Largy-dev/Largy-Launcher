import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

const prefersDark = window.matchMedia("(prefers-color-scheme: dark)");
document.documentElement.classList.toggle("dark", prefersDark.matches);
prefersDark.addEventListener("change", (e) => {
  document.documentElement.classList.toggle("dark", e.matches);
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
