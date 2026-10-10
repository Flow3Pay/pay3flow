/** Keep list navigation and tab focus inside the open selection dialog. */
export function navigatePicker(event: KeyboardEvent, dialog: HTMLElement | undefined) {
  if (!dialog || !dialog.contains(document.activeElement)) return;
  if (event.key === "Tab") {
    const controls = [...dialog.querySelectorAll<HTMLElement>('button:not(:disabled), input, [tabindex="0"]')].filter(node => node.checkVisibility());
    const first = controls[0], last = controls.at(-1);
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    return;
  }
  if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
  const options = [...dialog.querySelectorAll<HTMLElement>('[role="option"]')];
  if (!options.length) return;
  const current = options.indexOf(document.activeElement as HTMLElement);
  if ((event.key === "Home" || event.key === "End") && current < 0) return;
  event.preventDefault();
  const index = event.key === "Home" ? 0 : event.key === "End" ? options.length - 1 : event.key === "ArrowDown" ? (current + 1) % options.length : current <= 0 ? options.length - 1 : current - 1;
  options[index].focus();
}
