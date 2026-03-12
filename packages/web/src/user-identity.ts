const COLORS = [
  '#e06c75',
  '#e5c07b',
  '#98c379',
  '#56b6c2',
  '#61afef',
  '#c678dd',
  '#d19a66',
  '#be5046',
  '#7ec699',
  '#f472b6',
  '#a78bfa',
  '#fb923c',
];

const ADJECTIVES = [
  'Bold',
  'Brave',
  'Bright',
  'Calm',
  'Clever',
  'Cool',
  'Crimson',
  'Daring',
  'Eager',
  'Fast',
  'Fierce',
  'Golden',
  'Happy',
  'Keen',
  'Kind',
  'Lively',
  'Lucky',
  'Noble',
  'Quick',
  'Sharp',
  'Silent',
  'Silver',
  'Swift',
  'Vivid',
  'Wise',
];

const NOUNS = [
  'Bear',
  'Crane',
  'Crow',
  'Deer',
  'Eagle',
  'Falcon',
  'Fox',
  'Hawk',
  'Heron',
  'Lion',
  'Lynx',
  'Otter',
  'Owl',
  'Panda',
  'Raven',
  'Robin',
  'Shark',
  'Swan',
  'Tiger',
  'Whale',
  'Wolf',
  'Wren',
];

export interface UserIdentity {
  name: string;
  color: string;
}

const STORAGE_NAME_KEY = 'cadmus-user-name';
const STORAGE_COLOR_KEY = 'cadmus-user-color';

function pickRandom<T>(arr: T[]): T {
  return arr[Math.floor(Math.random() * arr.length)];
}

export function getOrCreateUserIdentity(): UserIdentity {
  // Use sessionStorage so each tab gets its own identity,
  // while still persisting across reloads of the same tab.
  const storedName = sessionStorage.getItem(STORAGE_NAME_KEY);
  const storedColor = sessionStorage.getItem(STORAGE_COLOR_KEY);

  if (storedName && storedColor) {
    return { name: storedName, color: storedColor };
  }

  const name = `${pickRandom(ADJECTIVES)} ${pickRandom(NOUNS)}`;
  const color = pickRandom(COLORS);

  sessionStorage.setItem(STORAGE_NAME_KEY, name);
  sessionStorage.setItem(STORAGE_COLOR_KEY, color);

  return { name, color };
}

export function clearUserIdentity(): void {
  sessionStorage.removeItem(STORAGE_NAME_KEY);
  sessionStorage.removeItem(STORAGE_COLOR_KEY);
}
