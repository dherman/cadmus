import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import {
  loginUser,
  registerUser,
  refreshToken,
  fetchWsToken,
  fetchMe,
  setAccessTokenProvider,
} from '../api';
import type { UserProfile } from '../api';

const TOKEN_KEY = 'cadmus-access-token';
const REFRESH_KEY = 'cadmus-refresh-token';
const EXPIRY_KEY = 'cadmus-token-expiry';

interface AuthContextValue {
  user: UserProfile | null;
  isLoading: boolean;
  login: (email: string, password: string) => Promise<void>;
  register: (email: string, displayName: string, password: string) => Promise<void>;
  logout: () => void;
  getAccessToken: () => Promise<string>;
  getWsToken: () => Promise<string>;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
}

function storeTokens(accessToken: string, refreshTokenValue: string, expiresIn: number) {
  localStorage.setItem(TOKEN_KEY, accessToken);
  localStorage.setItem(REFRESH_KEY, refreshTokenValue);
  localStorage.setItem(EXPIRY_KEY, String(Date.now() + expiresIn * 1000));
}

function clearTokens() {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(REFRESH_KEY);
  localStorage.removeItem(EXPIRY_KEY);
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<UserProfile | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  // Attempt to restore session on mount
  useEffect(() => {
    async function restore() {
      const storedToken = localStorage.getItem(TOKEN_KEY);
      const storedRefresh = localStorage.getItem(REFRESH_KEY);
      const storedExpiry = localStorage.getItem(EXPIRY_KEY);

      if (!storedToken || !storedRefresh || !storedExpiry) {
        setIsLoading(false);
        return;
      }

      try {
        const expiry = Number(storedExpiry);
        let token = storedToken;

        // If expired or expiring soon, try refresh
        if (Date.now() > expiry - 60_000) {
          const res = await refreshToken(storedRefresh);
          token = res.access_token;
          storeTokens(token, storedRefresh, res.expires_in);
        }

        const profile = await fetchMe(token);
        setUser(profile);
      } catch {
        clearTokens();
      } finally {
        setIsLoading(false);
      }
    }
    restore();
  }, []);

  const getAccessToken = useCallback(async (): Promise<string> => {
    const storedToken = localStorage.getItem(TOKEN_KEY);
    const storedRefresh = localStorage.getItem(REFRESH_KEY);
    const storedExpiry = localStorage.getItem(EXPIRY_KEY);

    if (!storedToken || !storedRefresh || !storedExpiry) {
      throw new Error('Not authenticated');
    }

    const expiry = Number(storedExpiry);

    // Proactively refresh if expiring within 60s
    if (Date.now() > expiry - 60_000) {
      const res = await refreshToken(storedRefresh);
      storeTokens(res.access_token, storedRefresh, res.expires_in);
      return res.access_token;
    }

    return storedToken;
  }, []);

  // Wire up the token provider for authFetch
  useEffect(() => {
    setAccessTokenProvider(getAccessToken);
  }, [getAccessToken]);

  const getWsToken = useCallback(async (): Promise<string> => {
    const token = await getAccessToken();
    const res = await fetchWsToken(token);
    return res.ws_token;
  }, [getAccessToken]);

  const login = useCallback(async (email: string, password: string) => {
    const res = await loginUser(email, password);
    storeTokens(res.access_token, res.refresh_token, res.expires_in);
    setUser(res.user);
  }, []);

  const register = useCallback(async (email: string, displayName: string, password: string) => {
    const res = await registerUser(email, displayName, password);
    storeTokens(res.access_token, res.refresh_token, res.expires_in);
    setUser(res.user);
  }, []);

  const logout = useCallback(() => {
    clearTokens();
    setUser(null);
  }, []);

  const value = useMemo(
    () => ({
      user,
      isLoading,
      login,
      register,
      logout,
      getAccessToken,
      getWsToken,
    }),
    [user, isLoading, login, register, logout, getAccessToken, getWsToken],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}
