# PR 4: Frontend Auth UI — Implementation Plan

## Prerequisites

- [ ] PR 1 (Users & Auth Endpoints) merged — auth API available
- [ ] PR 2 (JWT Middleware) merged — document endpoints require auth

## Steps

### Step 1: Add auth API functions

- [ ] Add auth functions to `packages/web/src/api.ts`:

```typescript
export interface UserProfile {
  id: string;
  email: string;
  display_name: string;
}

export interface AuthResponse {
  user: UserProfile;
  access_token: string;
  refresh_token: string;
  expires_in: number;
}

export interface TokenResponse {
  access_token: string;
  expires_in: number;
}

export interface WsTokenResponse {
  ws_token: string;
  expires_in: number;
}

export async function registerUser(
  email: string,
  displayName: string,
  password: string,
): Promise<AuthResponse> {
  const res = await fetch(`${API_BASE}/api/auth/register`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      email,
      display_name: displayName,
      password,
    }),
  });
  if (!res.ok) {
    const body = await res.json();
    throw new Error(body.error || 'Registration failed');
  }
  return res.json();
}

export async function loginUser(email: string, password: string): Promise<AuthResponse> {
  /* similar pattern */
}

export async function refreshToken(refreshToken: string): Promise<TokenResponse> {
  /* similar pattern */
}

export async function fetchWsToken(accessToken: string): Promise<WsTokenResponse> {
  /* similar pattern, with Authorization header */
}

export async function fetchMe(accessToken: string): Promise<UserProfile> {
  /* GET /api/auth/me with Authorization header */
}
```

### Step 2: Create AuthContext

- [ ] Create `packages/web/src/auth/AuthContext.tsx`:

```typescript
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
```

- [ ] Implement `AuthProvider` component:
  - On mount, check `localStorage` for stored tokens
  - If tokens exist, call `GET /api/auth/me` to validate
  - If expired, attempt refresh with stored refresh token
  - `getAccessToken()` proactively refreshes if token expires within 60 seconds
  - `getWsToken()` calls `POST /api/auth/ws-token` with current access token
  - `logout()` clears localStorage and resets state

### Step 3: Create LoginPage

- [ ] Create `packages/web/src/auth/LoginPage.tsx`:
  - Form with email + password inputs
  - Submit calls `useAuth().login()`
  - On success, navigate to `/`
  - On error, display error message
  - Link to `/register`
  - If already authenticated, redirect to `/`

### Step 4: Create RegisterPage

- [ ] Create `packages/web/src/auth/RegisterPage.tsx`:
  - Form with email + display name + password inputs
  - Client-side validation (email contains `@`, display name non-empty, password ≥ 8 chars)
  - Submit calls `useAuth().register()`
  - On success, navigate to `/`
  - On error, display error message
  - Link to `/login`
  - If already authenticated, redirect to `/`

### Step 5: Create ProtectedRoute

- [ ] Create `packages/web/src/auth/ProtectedRoute.tsx`:

```tsx
export function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { user, isLoading } = useAuth();

  if (isLoading) {
    return <div className="loading">Loading...</div>;
  }

  if (!user) {
    return <Navigate to="/login" replace />;
  }

  return <>{children}</>;
}
```

### Step 6: Update routing in main.tsx

- [ ] Wrap the app in `AuthProvider`
- [ ] Add `/login` and `/register` routes
- [ ] Wrap dashboard and editor routes in `ProtectedRoute`

```tsx
<BrowserRouter>
  <AuthProvider>
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route path="/register" element={<RegisterPage />} />
      <Route
        path="/"
        element={
          <ProtectedRoute>
            <Dashboard />
          </ProtectedRoute>
        }
      />
      <Route
        path="/docs/:id"
        element={
          <ProtectedRoute>
            <EditorPage />
          </ProtectedRoute>
        }
      />
    </Routes>
  </AuthProvider>
</BrowserRouter>
```

### Step 7: Update api.ts to use auth headers

- [ ] Create an `authFetch` wrapper that adds the `Authorization` header:

```typescript
let getAccessTokenFn: (() => Promise<string>) | null = null;

export function setAccessTokenProvider(fn: () => Promise<string>) {
  getAccessTokenFn = fn;
}

async function authFetch(url: string, options: RequestInit = {}): Promise<Response> {
  if (!getAccessTokenFn) throw new Error('Auth not initialized');
  const token = await getAccessTokenFn();
  return fetch(url, {
    ...options,
    headers: {
      ...options.headers,
      Authorization: `Bearer ${token}`,
    },
  });
}
```

- [ ] Update `listDocuments`, `createDocument`, `getDocument` to use `authFetch`
- [ ] `AuthContext` calls `setAccessTokenProvider` on init

### Step 8: Update user-identity.ts

- [ ] Replace `getOrCreateUserIdentity()` with `getUserIdentity(user: UserProfile)`:

```typescript
function hashCode(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash |= 0;
  }
  return hash;
}

export function getUserIdentity(user: UserProfile): UserIdentity {
  const colorIndex = Math.abs(hashCode(user.id)) % COLORS.length;
  return {
    name: user.display_name,
    color: COLORS[colorIndex],
  };
}
```

- [ ] Keep `getOrCreateUserIdentity` temporarily as a fallback during transition, or remove and update all call sites

### Step 9: Update EditorPage to use ws-token

- [ ] In `EditorPage.tsx` (or `useCollaboration.ts`), get a ws-token before connecting:

```typescript
const { getWsToken, user } = useAuth();

useEffect(() => {
  async function connect() {
    const wsToken = await getWsToken();
    const { ydoc, provider } = createCollaborationProvider(docId, wsToken);
    // ... rest of setup
  }
  connect();
}, [docId]);
```

- [ ] Handle WebSocket close code `4401` for token renewal:

```typescript
provider.ws?.addEventListener('close', async (event) => {
  if (event.code === 4401) {
    const newToken = await getWsToken();
    // Update provider URL and reconnect
    provider.url = `${WS_BASE_URL}/${docId}/ws?token=${encodeURIComponent(newToken)}`;
    provider.connect();
  }
});
```

### Step 10: Update Dashboard with user header

- [ ] Add a navigation bar showing the user's display name and a "Log out" button:

```tsx
const { user, logout } = useAuth();

return (
  <div>
    <header>
      <h1>Cadmus</h1>
      <div>
        <span>{user?.display_name}</span>
        <button onClick={logout}>Log out</button>
      </div>
    </header>
    {/* existing dashboard content */}
  </div>
);
```

### Step 11: Update awareness to use real user identity

- [ ] In the collaboration setup, use `getUserIdentity(user)` instead of `getOrCreateUserIdentity()`:

```typescript
const identity = getUserIdentity(user);
provider.awareness.setLocalStateField('user', {
  id: user.id,
  name: identity.name,
  color: identity.color,
});
```

## Verification

- [ ] Visiting `/` without logging in redirects to `/login`
- [ ] Registration creates a user and redirects to the dashboard
- [ ] Login works with valid credentials, shows error for invalid
- [ ] Dashboard shows user name and logout button
- [ ] Document list loads (requires auth — server filters by permission)
- [ ] Opening a document connects WebSocket with ws-token
- [ ] Awareness shows real user name instead of random animal name
- [ ] Logging out clears state and redirects to `/login`
- [ ] Refreshing the page maintains the logged-in state
- [ ] Long sessions survive token refresh (access token auto-renews)
- [ ] `pnpm run format:check` passes
- [ ] `pnpm run build` succeeds (web package)

## Files Created/Modified

- `packages/web/src/auth/AuthContext.tsx` (new)
- `packages/web/src/auth/LoginPage.tsx` (new)
- `packages/web/src/auth/RegisterPage.tsx` (new)
- `packages/web/src/auth/ProtectedRoute.tsx` (new)
- `packages/web/src/api.ts` (modified — auth functions, authFetch wrapper)
- `packages/web/src/main.tsx` (modified — auth routes, AuthProvider)
- `packages/web/src/user-identity.ts` (modified — real user identity)
- `packages/web/src/collaboration.ts` (modified — already updated in PR 3)
- `packages/web/src/Dashboard.tsx` (modified — user header, logout)
- `packages/web/src/EditorPage.tsx` (modified — ws-token flow)
- `packages/web/src/useCollaboration.ts` (modified — real user identity in awareness)
