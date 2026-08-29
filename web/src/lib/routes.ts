export type ProtectedRoute = 'home' | 'notes' | 'library' | 'tasks' | 'archive';
export type AppRoute = ProtectedRoute | 'login';

export function routeFromPath(pathname: string): AppRoute {
  if (pathname === '/login') return 'login';
  if (pathname === '/notes') return 'notes';
  if (pathname === '/library') return 'library';
  if (pathname === '/tasks') return 'tasks';
  if (pathname === '/archive') return 'archive';
  return 'home';
}

export function pathForRoute(route: AppRoute): string {
  if (route === 'login') return '/login';
  if (route === 'notes') return '/notes';
  if (route === 'library') return '/library';
  if (route === 'tasks') return '/tasks';
  if (route === 'archive') return '/archive';
  return '/';
}

export function safeReturnPath(pathname: string): string {
  return ['/', '/notes', '/library', '/tasks', '/archive'].includes(pathname) ? pathname : '/';
}
