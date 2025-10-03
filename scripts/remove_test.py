def sort_list(lst):
    """
    Sort a list in ascending order.

    Args:
        lst: A list of comparable elements

    Returns:
        A new sorted list
    """
    return sorted(lst)


def sort_list_in_place(lst):
    """
    Sort a list in place (modifies the original list).

    Args:
        lst: A list of comparable elements

    Returns:
        None (modifies the list in place)
    """
    lst.sort()


def sort_list_descending(lst):
    """
    Sort a list in descending order.

    Args:
        lst: A list of comparable elements

    Returns:
        A new sorted list in descending order
    """
    return sorted(lst, reverse=True)


def sum_two_integers(a, b):
    """
    Sum two integers.

    Args:
        a: First integer
        b: Second integer

    Returns:
        The sum of a and b
    """
    return a + b


def subtract_two_integers(a, b):
    """
    Subtract two integers.

    Args:
        a: First integer
        b: Second integer

    Returns:
        The difference of a and b (a - b)
    """
    return a - b


if __name__ == "__main__":
    # Example usage
    numbers = [64, 34, 25, 12, 22, 11, 90]

    print("Original list:", numbers)
    print("Sorted (ascending):", sort_list(numbers))
    print("Sorted (descending):", sort_list_descending(numbers))

    # In-place sorting
    numbers_copy = numbers.copy()
    sort_list_in_place(numbers_copy)
    print("Sorted in-place:", numbers_copy)


def multiply_two_integers(a, b):
    """
    Multiply two integers.

    Args:
        a: First integer
        b: Second integer

    Returns:
        The product of a and b
    """
    return a * b


def divide_two_integers(a, b):
    """
    Divide two integers.

    Args:
        a: First integer (dividend)
        b: Second integer (divisor)

    Returns:
        The quotient of a divided by b

    Raises:
        ZeroDivisionError: If b is zero
    """
    if b == 0:
        raise ZeroDivisionError("Cannot divide by zero")
    return a / b
