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

export interface ApiErrorPayload {
  error: {
    code: string;
    message: string;
  };
}
