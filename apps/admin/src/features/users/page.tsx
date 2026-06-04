import { useUsersQuery } from "./queries";

export function UsersPage() {
  const users = useUsersQuery();

  return (
    <section>
      <h1>Users</h1>
      <ul>
        {users.data?.map((user) => (
          <li key={user.id}>{user.username}</li>
        ))}
      </ul>
    </section>
  );
}
