class Outer:
    """Outer class for scope testing"""

    class Inner:
        """Inner class nested inside Outer"""

        def inner_method(self):
            """Method inside Inner"""
            return True

    def outer_method(self):
        """Method on Outer"""
        inner = self.Inner()
        return inner.inner_method()
