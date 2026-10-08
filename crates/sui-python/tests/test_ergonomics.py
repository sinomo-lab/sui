import sinomo_ui as sui


def test_value_types_compare_and_hash_by_value():
    assert sui.Point(1, 2) == sui.Point(1, 2)
    assert sui.Point(1, 2) != sui.Point(2, 1)
    assert sui.Size(0.0, 1) == sui.Size(-0.0, 1)
    assert hash(sui.Size(0.0, 1)) == hash(sui.Size(-0.0, 1))
    assert sui.Rect(0, 0, 4, 4) == sui.Rect(0, 0, 4, 4)
    assert sui.Color.rgba(1, 0, 0, 1) == sui.Color.rgba(1, 0, 0, 1)
    assert len({sui.Point(1, 2), sui.Point(1, 2), sui.Point(3, 4)}) == 2
    assert sui.FontHandle(7) == sui.FontHandle(7)
    assert {sui.ImageHandle(1): "a"}[sui.ImageHandle(1)] == "a"
    assert sui.Point(1, 2) != (1, 2)


def test_reprs_describe_the_object():
    assert repr(sui.State(3)) == "State(3)"
    assert repr(sui.State("on")) == "State('on')"
    assert repr(sui.Window("Main")) == 'Window("Main")'
    assert repr(sui.label("x")) == "Widget(Label)"
    assert repr(sui.column([])) == "Widget(Flex)"
    assert repr(sui.App()) == "App(windows=0)"
    assert repr(sui.ScrollController()).startswith("ScrollController(offset=")


def test_state_subscription_is_a_context_manager():
    state = sui.State(1)
    seen = []
    with state.watch(seen.append) as subscription:
        state.set(2)
        assert "active=True" in repr(subscription)
    state.set(3)
    assert seen == [2]
    assert "active=False" in repr(subscription)
