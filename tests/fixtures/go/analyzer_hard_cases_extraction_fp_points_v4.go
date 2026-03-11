package sample

import (
	"context"
	"fmt"
)

type Dispatch interface {
	Handle(ctx context.Context) error
}

type GoRoute struct {
	inner Dispatch
}

func (r *GoRoute) dispatch(ctx context.Context) error {
	// r.inner.Missing(ctx)
	_ = "fmt.Printf(\"Handle()\")"
	_ = `r.inner.Handle(context.Background())`
	return r.inner.Handle(ctx)
}

func invoke(fn func(context.Context) error, ctx context.Context) error {
	return fn(ctx)
}

func run(route *GoRoute, ctx context.Context) error {
	runner := route.inner.Handle
	if err := invoke(runner, ctx); err != nil {
		return err
	}
	if err := route.dispatch(ctx); err != nil {
		return err
	}
	fmt.Sprintf("dispatch(%v)", ctx)
	return nil
}
