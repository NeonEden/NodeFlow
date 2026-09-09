import { UserProfile } from '../types';

const AUTH_STORAGE_KEY = 'neuralmind_auth_user';
const USERS_STORAGE_KEY = 'neuralmind_registered_users';

export const DEMO_ACCOUNTS: UserProfile[] = [
  {
    id: 'user-tomas',
    name: 'Tomás Pieruz',
    email: 'tomaspieruz@gmail.com',
    role: 'Innovador & Estratega IA',
    avatar: 'https://images.unsplash.com/photo-1534528741775-53994a69daeb?w=100&h=100&fit=crop&crop=faces',
  },
  {
    id: 'user-elena',
    name: 'Elena Rostova',
    email: 'elena@neuralmind.ai',
    role: 'Arquitecta de Conocimiento',
    avatar: 'https://images.unsplash.com/photo-1580489944761-15a19d654956?w=100&h=100&fit=crop&crop=faces',
  },
  {
    id: 'user-carlos',
    name: 'Carlos Méndez',
    email: 'carlos@techcorp.io',
    role: 'Ingeniero de Sistemas',
    avatar: 'https://images.unsplash.com/photo-1507003211169-0a1dd7228f2d?w=100&h=100&fit=crop&crop=faces',
  },
];

export const getInitialUser = (): UserProfile => {
  try {
    const stored = localStorage.getItem(AUTH_STORAGE_KEY);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (e) {
    console.error('Error reading auth user from localStorage', e);
  }
  return DEMO_ACCOUNTS[0]; // Default authenticated as Tomás
};

export const saveCurrentUser = (user: UserProfile | null): void => {
  try {
    if (user) {
      localStorage.setItem(AUTH_STORAGE_KEY, JSON.stringify(user));
    } else {
      localStorage.removeItem(AUTH_STORAGE_KEY);
    }
  } catch (e) {
    console.error('Error saving auth user to localStorage', e);
  }
};

export const getRegisteredUsers = (): UserProfile[] => {
  try {
    const stored = localStorage.getItem(USERS_STORAGE_KEY);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (e) {
    console.error('Error reading registered users', e);
  }
  return DEMO_ACCOUNTS;
};

export const registerUser = (name: string, email: string, role: string): UserProfile => {
  const users = getRegisteredUsers();
  const existing = users.find((u) => u.email.toLowerCase() === email.toLowerCase());
  if (existing) {
    saveCurrentUser(existing);
    return existing;
  }

  const newUser: UserProfile = {
    id: `user-${Date.now()}`,
    name,
    email,
    role: role || 'Investigador',
    avatar: `https://api.dicebear.com/7.x/bottts/svg?seed=${encodeURIComponent(name)}`,
  };

  const updatedUsers = [...users, newUser];
  try {
    localStorage.setItem(USERS_STORAGE_KEY, JSON.stringify(updatedUsers));
  } catch (e) {
    console.error('Error persisting new user', e);
  }

  saveCurrentUser(newUser);
  return newUser;
};
