import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "./ajustes.css";
import { render } from "solid-js/web";
import { connect } from "../lib/api";
import { initState } from "../lib/state";
import { Ajustes } from "./Ajustes";

initState();
void connect();
render(() => <Ajustes />, document.getElementById("root")!);
