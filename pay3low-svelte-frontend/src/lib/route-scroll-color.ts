/** Keep the native scrollbar in sync with the position in the ranked routes. */
export function routeScrollColor(node: HTMLElement) {
  const colors = [[181, 245, 0], [250, 204, 21], [249, 115, 22], [239, 68, 68]];
  const update = () => {
    const extent = node.scrollHeight - node.clientHeight;
    const position = extent > 0 ? Math.min(3, Math.max(0, node.scrollTop / extent * 3)) : 0;
    const index = Math.min(2, Math.floor(position));
    const color = colors[index].map((value, channel) => Math.round(value + (colors[index + 1][channel] - value) * (position - index)));
    node.style.setProperty("--route-scroll-color", `rgb(${color.join(", ")})`);
  };
  const resize = new ResizeObserver(update);
  resize.observe(node);
  if (node.firstElementChild) resize.observe(node.firstElementChild);
  const mutation = new MutationObserver(update);
  mutation.observe(node, { childList: true, subtree: true });
  node.addEventListener("scroll", update, { passive: true });
  update();
  return { destroy() { resize.disconnect(); mutation.disconnect(); node.removeEventListener("scroll", update); } };
}
