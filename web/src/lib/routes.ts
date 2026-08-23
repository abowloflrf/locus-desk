export type ProtectedRoute = 'home' | 'tasks' | 'archive';
export type AppRoute = ProtectedRoute | 'login';

export function routeFromPath(pathname: string): AppRoute {
  if (pathname === '/login') return 'login';
  if (pathname === '/tasks') return 'tasks';
  if (pathname === '/archive') return 'archive';
  return 'home';
}

export function pathForRoute(route: AppRoute): string {
  if (route === 'login') return '/login';
  if (route === 'tasks') return '/tasks';
  if (route === 'archive') return '/archive';
  return '/';
}

export function safeReturnPath(pathname: string): string {
  return ['/', '/tasks', '/archive'].includes(pathname) ? pathname : '/';
}
