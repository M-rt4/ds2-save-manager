import { Component, type ReactNode } from 'react';

/** Keeps a failing panel from blanking the whole window; shows what broke instead. */
export class ErrorBoundary extends Component<{ children: ReactNode; resetKey?: unknown }, { error: Error | null }> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidUpdate(prev: { resetKey?: unknown }) {
    if (prev.resetKey !== this.props.resetKey && this.state.error) this.setState({ error: null });
  }

  render() {
    if (this.state.error) {
      return (
        <div className="warn" style={{ userSelect: 'text' }}>
          {this.state.error.message}
        </div>
      );
    }
    return this.props.children;
  }
}
