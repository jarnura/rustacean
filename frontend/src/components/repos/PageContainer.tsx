export function PageContainer({
  children,
}: {
  readonly children: React.ReactNode;
}): JSX.Element {
  return <div className="container max-w-3xl py-8">{children}</div>;
}
