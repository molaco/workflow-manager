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
