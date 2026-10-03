import { request } from './client';

export function createTask(token, task) {
  return request('/tasks', { method: 'POST', body: task, token });
}

export function listAllTasks(token) {
  return request('/tasks', { token });
}

export function assignTasks(token, taskIds, assigneeEmail) {
  return request('/tasks/assign', {
    method: 'POST',
    body: { task_ids: taskIds, assignee_email: assigneeEmail },
    token,
  });
}

export function viewMyTasks(token) {
  return request('/tasks/view-my-tasks', { token });
}
