import { useUsersQuery } from "./queries";

export function UsersPage() {
  const users = useUsersQuery();
  const userList = users.data ?? [];

  return (
    <section className="stack">
      <h1>Users</h1>
      <div className="stats">
        <span>{userList.length} users</span>
      </div>
      <ul className="results-list">
        {userList.map((user) => (
          <li key={user.id} className="card stack compact panel">
            <strong>{user.username}</strong>
            <span>{user.role}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
