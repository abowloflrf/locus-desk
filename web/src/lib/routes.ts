export type ProtectedRoute = 'home' | 'notes' | 'library' | 'tasks' | 'archive' | 'settings';
export type AppRoute = ProtectedRoute | 'login';

export function routeFromPath(pathname: string): AppRoute {
  if (pathname === '/settings') return 'settings';
  if (pathname === '/login') return 'login';
  if (pathname === '/notes') return 'notes';
  if (pathname === '/library') return 'library';
  if (pathname === '/tasks') return 'tasks';
  if (pathname === '/archive') return 'archive';
  return 'home';
}

export function pathForRoute(route: AppRoute): string {
  if (route === 'settings') return '/settings';
  if (route === 'login') return '/login';
  if (route === 'notes') return '/notes';
  if (route === 'library') return '/library';
  if (route === 'tasks') return '/tasks';
  if (route === 'archive') return '/archive';
  return '/';
}

export function safeReturnPath(pathname: string): string {
  return ['/', '/notes', '/library', '/tasks', '/archive', '/settings'].includes(pathname)
    ? pathname
    : '/';
}
