import { request } from './client';

export function login(email, password) {
  return request('/auth/login', { method: 'POST', body: { email, password } });
}

export function verify2fa(loginChallengeId, code) {
  return request('/auth/verify-2fa', {
    method: 'POST',
    body: { login_challenge_id: loginChallengeId, code },
  });
}

/** Dev-only helper: read the latest "email" from the local inbox. */
export function latestEmailLog(email) {
  return request(`/dev/email-logs/latest?email=${encodeURIComponent(email)}`);
}
