export function cursorBuilder(user: { name: string; color: string }): HTMLElement {
  const cursor = document.createElement('span');
  cursor.classList.add('collaboration-cursor');
  cursor.style.borderColor = user.color;

  const label = document.createElement('span');
  label.classList.add('collaboration-cursor-label');
  label.style.backgroundColor = user.color;
  label.textContent = user.name;

  cursor.appendChild(label);
  return cursor;
}
