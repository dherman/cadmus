# PR 4: Frontend Auth UI

## Purpose

Add login and registration screens to the frontend, manage auth state in React context, and wire all API calls to include authentication headers. After this PR, users must log in before accessing the dashboard or editor. The anonymous random-identity system is replaced with real user accounts.

## Auth Flow (User Perspective)

1. User visits the app → redirected to `/login` if no stored token.
2. User registers or logs in → tokens stored in `localStorage`.
3. Dashboard loads, showing only documents the user has access to.
4. User opens a document → frontend fetches a ws-token and connects the WebSocket.
5. If the access token expires, the refresh token silently renews it.
6. If the ws-token expires (WebSocket close code `4401`), the frontend fetches a new one and reconnects.
7. User clicks "Log out" → tokens cleared, redirected to `/login`.

## New Components

### AuthContext (`auth/AuthContext.tsx`)

React context that provides:

```typescript
interface AuthContextValue {
  user: UserProfile | null;
  isLoading: boolean;
  login: (email: string, password: string) => Promise<void>;
  register: (email: string, displayName: string, password: string) => Promise<void>;
  logout: () => void;
  getAccessToken: () => Promise<string>; // handles refresh transparently
  getWsToken: () => Promise<string>;
}
```

**Token storage:** Access token, refresh token, and expiry timestamp stored in `localStorage`. On app load, `AuthContext` checks for a stored token and validates it via `GET /api/auth/me`. If valid, the user is logged in. If expired, a refresh is attempted.

**Token refresh logic:** `getAccessToken()` checks the stored expiry. If the token expires within 60 seconds, it proactively refreshes before returning. This prevents requests from failing due to a token that expires between the check and the server processing the request.

### LoginPage (`auth/LoginPage.tsx`)

- Email and password inputs
- "Log in" button
- Link to registration page
- Error display for invalid credentials
- Redirects to `/` (dashboard) on success

### RegisterPage (`auth/RegisterPage.tsx`)

- Email, display name, and password inputs
- "Create account" button
- Link to login page
- Validation: email format, display name non-empty, password ≥ 8 chars (client-side, server validates too)
- Error display for duplicate email, validation failures
- Redirects to `/` (dashboard) on success (auto-logged-in via the returned tokens)

### ProtectedRoute (`auth/ProtectedRoute.tsx`)

Route wrapper that redirects to `/login` if the user is not authenticated:

```tsx
function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { user, isLoading } = useAuth();

  if (isLoading) return <LoadingSpinner />;
  if (!user) return <Navigate to="/login" />;

  return <>{children}</>;
}
```

## Changes to Existing Components

### `api.ts`

All fetch calls gain an `Authorization: Bearer {token}` header:

```typescript
async function authFetch(url: string, options: RequestInit = {}): Promise<Response> {
  const token = await getAccessToken();
  return fetch(url, {
    ...options,
    headers: {
      ...options.headers,
      Authorization: `Bearer ${token}`,
    },
  });
}
```

Replace all `fetch` calls in the existing API functions with `authFetch`. The `getAccessToken` function is provided by the `AuthContext` — `api.ts` imports it or receives it as a parameter.

### `collaboration.ts`

The `createCollaborationProvider` function now requires a ws-token (signature changed in PR 3). The `EditorPage` component calls `getWsToken()` from `AuthContext` before creating the provider.

Handle WebSocket close code `4401`:

```typescript
provider.on('status', ({ status }: { status: string }) => {
  if (status === 'disconnected') {
    // Check if disconnect was due to token expiry
    // If so, get a new ws-token and update the provider URL
  }
});
```

### `user-identity.ts`

The random name/color generator is replaced. User identity now comes from the auth context:

- `name` → user's `display_name` from the JWT/profile
- `color` → deterministically derived from user ID (hash → index into color palette), ensuring consistent colors across sessions

The `getOrCreateUserIdentity()` function is refactored to accept a user profile and return an identity:

```typescript
export function getUserIdentity(user: UserProfile): UserIdentity {
  const colorIndex = hashCode(user.id) % COLORS.length;
  return {
    name: user.display_name,
    color: COLORS[Math.abs(colorIndex)],
  };
}
```

### `main.tsx`

Route structure changes:

```tsx
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
```

### `Dashboard.tsx`

- Shows the logged-in user's name in a header/nav bar
- "Log out" button
- Document list now only shows documents the user has access to (server filters via permission query)

## Styling

Login and registration pages use the same minimal styling as the existing app — no CSS framework. Simple centered card layout with form inputs. Consistent with the prototype aesthetic.

## Error Handling

| Scenario                         | UX Behavior                                          |
| -------------------------------- | ---------------------------------------------------- |
| Invalid login credentials        | Red error text below the form                        |
| Registration: duplicate email    | Error text: "Email already registered"               |
| Registration: password too short | Error text: "Password must be at least 8 characters" |
| Network error                    | Generic error text: "Something went wrong"           |
| Expired session (refresh fails)  | Redirect to login with message "Session expired"     |
| 401 on any API call              | Attempt refresh; if that fails, redirect to login    |

## What's Not Included

- **Document sharing UI** — PR 5 adds the sharing dialog.
- **Permission-aware editor controls** — PR 5 disables toolbar for read-only users.
- **Password reset** — deferred.
- **Social login / OAuth** — deferred to M7.
