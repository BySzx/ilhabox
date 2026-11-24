import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "./box.css";
import { render } from "solid-js/web";
import { connect } from "../lib/api";
import { initState } from "../lib/state";
import { App } from "./App";

initState();
void connect();
render(() => <App />, document.getElementById("root")!);
