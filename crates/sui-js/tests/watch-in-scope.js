export function watchInScope(state, listener, body) {
  using subscription = state.watch(listener);
  body();
}
