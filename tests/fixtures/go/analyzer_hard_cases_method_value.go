package sample

import "context"

type Runner interface {
    Run(ctx context.Context) error
}

type GenericService[T any] struct {
    next Runner
}

func (s *GenericService[T]) Execute(v T) error {
    _ = v
    invoke := s.next.Run
    if err := invoke(context.Background()); err != nil {
        return err
    }
    return s.next.Run(context.Background())
}
