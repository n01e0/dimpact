import React, { useMemo } from "react";

type Props<T> = {
  item: T;
  render: (v: T) => JSX.Element;
  onPick?: (v: T) => void;
};

export function Panel<T>(props: Props<T>) {
  const pick = props.onPick;

  const handle = useMemo(() => {
    return (v: T) => {
      pick?.(v);
      props.onPick?.(v);
      return props.render(v);
    };
  }, [pick, props]);

  return <section>{handle(props.item)}</section>;
}
