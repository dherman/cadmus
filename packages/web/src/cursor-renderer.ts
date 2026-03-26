export function cursorBuilder(user: {
  name: string;
  color: string;
  isAgent?: boolean;
}): HTMLElement {
  const cursor = document.createElement('span');
  cursor.classList.add('collaboration-cursor');
  if (user.isAgent) {
    cursor.setAttribute('data-agent', 'true');
    cursor.style.borderColor = '#888';
    cursor.style.borderLeftStyle = 'dashed';
  } else {
    cursor.style.borderColor = user.color;
  }

  const label = document.createElement('span');
  label.classList.add('collaboration-cursor-label');
  label.style.backgroundColor = user.isAgent ? '#888' : user.color;
  label.textContent = user.isAgent ? `🤖 ${user.name}` : user.name;

  cursor.appendChild(label);
  return cursor;
}
