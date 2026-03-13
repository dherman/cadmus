import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { createBrowserRouter, RouterProvider } from 'react-router';
import { Dashboard } from './Dashboard';
import { EditorPage } from './EditorPage';
import './editor.css';

const router = createBrowserRouter([
  { path: '/', element: <Dashboard /> },
  { path: '/docs/:id', element: <EditorPage /> },
]);

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
);
