interface Page<T> {
  items: T[];
  page: number;
  pageSize: number;
  total: number;
}

export type ListLoad = 'reset' | 'append' | 'refresh';
export type ListContinuationError = {
  load: Exclude<ListLoad, 'reset'>;
  message: string;
  lastPage: number;
};

export function mergePageItems<T extends { uid: string }>(current: T[], next: T[]): T[] {
  return [...new Map([...current, ...next].map((item) => [item.uid, item])).values()];
}

export async function fetchLoadedPages<T extends { uid: string }>(
  lastPage: number,
  fetchPage: (page: number) => Promise<Page<T>>,
  signal: AbortSignal,
): Promise<Page<T>> {
  let response = await fetchPage(1);
  const items = [...response.items];
  for (let page = 2; page <= lastPage && items.length < response.total; page += 1) {
    signal.throwIfAborted();
    response = await fetchPage(page);
    if (response.items.length === 0) break;
    items.push(...response.items);
  }
  return { ...response, items: mergePageItems([], items) };
}
