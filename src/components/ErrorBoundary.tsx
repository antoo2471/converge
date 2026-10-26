import { Component, type ReactNode } from "react";
export class ErrorBoundary extends Component<
  { children: ReactNode },
  { error: string }
> {
  state = { error: "" };
  static getDerivedStateFromError(error: Error) {
    return { error: error.message };
  }
  render() {
    return this.state.error ? (
      <div className="render-error" role="alert">
        <h2>Ce contenu n’a pas pu être affiché.</h2>
        <p>{this.state.error}</p>
        <button className="button" onClick={() => this.setState({ error: "" })}>
          Réessayer l’affichage
        </button>
      </div>
    ) : (
      this.props.children
    );
  }
}
