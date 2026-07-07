---
name: react
description: React best practices for this project. Use when writing or reviewing React components, hooks, event handlers, state management, or conditional rendering.
---

# React Best Practices

## Component Structure

Use functional components with TypeScript. Enforce immutability of props by wrapping the props type in `Readonly<>` on the component's argument. Define the type for component's props in the same file as the component. Prefer named exports for both the component and corresponding props type.

```typescript
// ✅ GOOD
export interface UserCardProps {
  name: string;
  email: string;
  onEdit?: () => void;
}

export const UserCard = ({ name, email, onEdit }: Readonly<UserCardProps>) => {
  return <Box>...</Box>;
}

// ❌ BAD - Props type unnamed and not exported
export const UserCard = ({ name, email, onEdit }: { name: string, email: string, onEdit: () => void }) => {
  return <Box>...</Box>;
}
```

## Hooks Usage

- Prefer `useCallback` and `useMemo` to control chain of updates, maintain clear code organization, and mitigate re-rendering issues.
- Bear in mind that for `useState` without arguments, the default value is `undefined` and the type parameter doesn't need to include `| undefined` union.
- Define invariants and local pure functions at module scope.
- Extract custom hooks for reusable logic:

```typescript
// ✅ GOOD
const useUserData = (userId: string) => {
  const [user, setUser] = useState<User>();
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetchUser(userId)
      .then(setUser)
      .finally(() => setLoading(false));
  }, [userId]);

  return { user, loading };
};

// Usage
const { user, loading } = useUserData(userId);

// ❌ BAD - Duplicate logic across components
const [user, setUser] = useState<User>();
useEffect(() => {
  /* fetch logic */
}, []);
```

## State Management

Keep state close to where it's used:

```typescript
// ✅ GOOD - Local state for local concerns
const [isOpen, setIsOpen] = useState(false);

// ✅ GOOD - Context for shared state
const { user, setUser } = useAuth();

// ❌ BAD - Prop drilling through many levels
<Parent user={user}>
  <Child user={user}>
    <GrandChild user={user} />
  </Child>
</Parent>
```

Prefer reducer patterns if the state is being updated from a nested component defined in another module.

## Event Handlers

Use proper TypeScript types for events:

```typescript
// ✅ GOOD
const handleClick = useCallback((event: React.MouseEvent<HTMLButtonElement>) => {
  event.preventDefault();
}, []);

const handleChange = useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
  setValue(event.target.value);
}, []);

// ❌ BAD
const handleClick = useCallback((event: any) => {}, []);
```

## Conditional Rendering

Use clear conditional patterns:

```typescript
// ✅ GOOD - Simple conditionals
{isLoading && <Spinner />}
{error && <ErrorMessage error={error} />}

// ✅ GOOD - Conditionals with 2 branches
{isLoading ? (
  <Spinner />
) : error ? (
  <ErrorMessage error={error} />
) : (
  <Content data={data} />
)}

// ✅ GOOD - Complex conditionals: extract to a variable with if/else
let content: ReactNode;
if (isLoading) {
  content = <Spinner />;
} else if (error) {
  content = <ErrorMessage error={error} />;
} else if (!data) {
  content = <EmptyState />;
} else {
  content = <Content data={data} />;
}

return <Container>{content}</Container>;

// ❌ BAD - Unclear nested ternaries
{isLoading ? <Spinner /> : error ? null : data ? <Content /> : null}
```
