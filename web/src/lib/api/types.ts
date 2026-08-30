export type WorkspaceRole = 'OWNER' | 'ADMIN' | 'MEMBER';

export interface SessionInfo {
  user: {
    uid: string;
    username: string;
  };
  workspace: {
    uid: string;
    name: string;
    timezone: string;
    today: string;
    role: WorkspaceRole;
  };
}

export interface LoginRequest {
  username: string;
  password: string;
}

export type NoteStatus = 'ACTIVE' | 'ARCHIVED';

export interface Note {
  uid: string;
  content: string;
  status: NoteStatus;
  pinned: boolean;
  tags: string[];
  createdAt: string;
  updatedAt: string;
}

export interface CreateNoteRequest {
  content: string;
}

export interface UpdateNoteRequest {
  content?: string;
  status?: NoteStatus;
  pinned?: boolean;
}

export interface ListNotesParams {
  status?: NoteStatus;
  q?: string;
  tag?: string;
  page?: number;
  pageSize?: number;
}

export interface ListNotesResponse {
  items: Note[];
  page: number;
  pageSize: number;
  total: number;
}

export type TaskStatus = 'TODO' | 'DONE';
export type TaskPriority = 0 | 1;

export interface Task {
  uid: string;
  title: string;
  description: string;
  status: TaskStatus;
  priority: TaskPriority;
  dueDate: string | null;
  dueTime: string | null;
  sortKey: number;
  completedAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreateTaskRequest {
  title: string;
  description?: string;
  priority?: TaskPriority;
  dueDate?: string;
  dueTime?: string;
}

export interface UpdateTaskRequest {
  title?: string;
  description?: string;
  status?: TaskStatus;
  priority?: TaskPriority;
  dueDate?: string | null;
  dueTime?: string | null;
  sortKey?: number;
}

export interface ListTasksParams {
  scope?: 'today';
  status?: TaskStatus;
}

export interface ListTasksResponse {
  items: Task[];
}

export interface ListTagsResponse {
  items: string[];
}

export type LibraryItemKind = 'BOOKMARK' | 'ARTICLE';
export type LibraryItemStatus = 'ACTIVE' | 'ARCHIVED';
export type LibraryProcessingStatus = 'NOT_FETCHED' | 'PENDING' | 'READY' | 'FAILED';
export type LibraryRefreshStatus = 'IDLE' | 'PENDING' | 'FAILED' | 'REVIEW';

export interface LibraryCapture {
  uid: string;
  selectedText: string;
  note: string;
  capturedTitle: string | null;
  createdAt: string;
}

export interface LibraryItem {
  uid: string;
  originalUrl: string;
  normalizedUrl: string;
  canonicalUrl: string | null;
  title: string;
  siteName: string | null;
  author: string | null;
  publishedAt: string | null;
  excerpt: string;
  itemKind: LibraryItemKind;
  status: LibraryItemStatus;
  readAt: string | null;
  starred: boolean;
  processingStatus: LibraryProcessingStatus;
  lastError: string | null;
  refreshStatus?: LibraryRefreshStatus;
  refreshError?: string | null;
  fetchedAt: string | null;
  contentVersion: number;
  contentAvailable: boolean;
  currentTextByteLen?: number | null;
  candidateContentVersion?: number | null;
  candidateTextByteLen?: number | null;
  tags: string[];
  captures: LibraryCapture[];
  createdAt: string;
  updatedAt: string;
}

export interface LibraryContentResponse {
  safeHtml: string;
  plainText: string;
  fetchedAt: string;
  contentVersion: number;
}

export interface CreateLibraryItemRequest {
  url: string;
  title?: string;
  selection?: string;
  note?: string;
  tags?: string[];
  idempotencyKey?: string;
}

export interface UpdateLibraryItemRequest {
  title?: string;
  status?: LibraryItemStatus;
  read?: boolean;
  starred?: boolean;
  tags?: string[];
}

export interface ListLibraryItemsParams {
  status?: LibraryItemStatus;
  q?: string;
  tag?: string;
  read?: boolean;
  starred?: boolean;
  page?: number;
  pageSize?: number;
}

export interface ListLibraryItemsResponse {
  items: LibraryItem[];
  page: number;
  pageSize: number;
  total: number;
}

export interface ApiErrorPayload {
  error: {
    code: string;
    message: string;
  };
}
